use crate::tds::Context;
use bytes::Buf;
use futures_util::io::AsyncRead;
use pin_project_lite::pin_project;
use std::io::ErrorKind::UnexpectedEof;
use std::{future::Future, io, mem::size_of, pin::Pin, task};
use task::Poll;

/// Largest up-front reservation (and per-step growth window) used when
/// reading a value whose length comes from the server. An untrusted length
/// can then never force a large allocation before the bytes actually arrive.
pub(crate) const MAX_PREALLOC: usize = 64 * 1024;

/// Appends exactly `len` bytes from `src` to `dst`, growing `dst` at most
/// [`MAX_PREALLOC`] bytes at a time so that a lying, server-supplied `len`
/// (up to 4 GiB for a u32, or u64::MAX for PLP) cannot reserve or zero-fill
/// memory for data that never arrives. A genuine long value still reads in
/// full. Adapted from upstream Tiberius's `read_bytes_into`
/// (tiberius-rs/tiberius 8255d35 / 3e6afc3).
pub(crate) async fn read_bytes_into<R>(src: &mut R, dst: &mut Vec<u8>, len: usize) -> io::Result<()>
where
    R: AsyncRead + Unpin,
{
    use futures_util::io::AsyncReadExt;

    let mut remaining = len;

    while remaining > 0 {
        let want = remaining.min(MAX_PREALLOC);
        let start = dst.len();
        let end = start
            .checked_add(want)
            .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "read length overflow"))?;
        dst.resize(end, 0);

        if let Err(e) = src.read_exact(&mut dst[start..]).await {
            dst.truncate(start);
            return Err(e);
        }

        // `want` is at most `remaining`, so this cannot fail.
        remaining = remaining
            .checked_sub(want)
            .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "read length underflow"))?;
    }

    Ok(())
}

macro_rules! varchar_reader {
    ($name:ident, $length_reader:ident) => {
        pin_project! {
            #[doc(hidden)]
            pub struct $name<R> {
                #[pin]
                src: R,
                length: Option<usize>,
                buf: Option<Vec<u16>>,
                read: usize
            }
        }

        #[allow(dead_code)]
        impl<R> $name<R> {
            pub(crate) fn new(src: R) -> Self {
                Self {
                    src,
                    length: None,
                    buf: None,
                    read: 0,
                }
            }
        }

        impl<R> Future for $name<R>
        where
            R: AsyncRead,
        {
            type Output = io::Result<String>;

            fn poll(self: Pin<&mut Self>, cx: &mut task::Context<'_>) -> Poll<Self::Output> {
                let mut me = self.project();

                // We must know the length of the string first.
                while me.length.is_none() {
                    let mut read_len = $length_reader::new(&mut me.src);

                    match Pin::new(&mut read_len).poll(cx) {
                        Poll::Pending => return Poll::Pending,
                        Poll::Ready(Err(e)) => return Poll::Ready(Err(e.into())),
                        Poll::Ready(Ok(length)) => {
                            let length = match usize::try_from(length) {
                                Ok(length) => length,
                                Err(_) => {
                                    return Poll::Ready(Err(io::Error::new(
                                        io::ErrorKind::InvalidData,
                                        "string length does not fit usize",
                                    )))
                                }
                            };
                            *me.length = Some(length);
                            *me.buf = Some(Vec::with_capacity(length));
                        }
                    }
                }

                // We've set the length and initialized the buffer
                let len = me.length.unwrap();
                let buf = me.buf.as_mut().unwrap();

                // Everything's read, we can return the string.
                if *me.read == len {
                    let s = String::from_utf16(&buf).map_err(|_| {
                        io::Error::new(io::ErrorKind::InvalidData, "Invalid UTF-16 data.")
                    })?;

                    return Poll::Ready(Ok(s));
                }

                // Read the utf-16 data
                while *me.read < len {
                    let mut read_u16 = ReadU16Le::new(&mut me.src);

                    match Pin::new(&mut read_u16).poll(cx) {
                        Poll::Pending => return Poll::Pending,
                        Poll::Ready(Err(e)) => return Poll::Ready(Err(e.into())),
                        Poll::Ready(Ok(n)) => {
                            buf.push(n);
                            *me.read += 1;
                        }
                    }
                }

                // Everything's read, we can return the string.
                let s = String::from_utf16(&buf).map_err(|_| {
                    io::Error::new(io::ErrorKind::InvalidData, "Invalid UTF-16 data.")
                })?;

                return Poll::Ready(Ok(s));
            }
        }
    };
}

macro_rules! bytes_reader {
    ($name:ident, $ty:ty, $reader:ident) => {
        bytes_reader!($name, $ty, $reader, size_of::<$ty>());
    };
    ($name:ident, $ty:ty, $reader:ident, $bytes:expr) => {
        pin_project! {
            #[doc(hidden)]
            pub struct $name<R> {
                #[pin]
                src: R,
                buf: [u8; $bytes],
                read: usize,
            }
        }

        #[allow(dead_code)]
        impl<R> $name<R> {
            pub(crate) fn new(src: R) -> Self {
                $name {
                    src,
                    buf: [0; $bytes],
                    read: 0,
                }
            }
        }

        impl<R> Future for $name<R>
        where
            R: AsyncRead,
        {
            type Output = io::Result<$ty>;

            fn poll(self: Pin<&mut Self>, cx: &mut task::Context<'_>) -> Poll<Self::Output> {
                let mut me = self.project();

                if *me.read == $bytes {
                    return Poll::Ready(Ok(Buf::$reader(&mut &me.buf[..])));
                }

                while *me.read < $bytes {
                    let read = match me.src.as_mut().poll_read(cx, &mut me.buf[*me.read..]) {
                        Poll::Pending => return Poll::Pending,
                        Poll::Ready(Err(e)) => return Poll::Ready(Err(e.into())),
                        Poll::Ready(Ok(0)) => {
                            return Poll::Ready(Err(UnexpectedEof.into()));
                        }
                        Poll::Ready(Ok(n)) => n,
                    };
                    *me.read = match me.read.checked_add(read) {
                        Some(total) => total,
                        None => {
                            return Poll::Ready(Err(io::Error::new(
                                io::ErrorKind::InvalidData,
                                "read counter overflow",
                            )))
                        }
                    };
                }

                let num = Buf::$reader(&mut &me.buf[..]);

                Poll::Ready(Ok(num))
            }
        }
    };
}

/// The `SqlReadBytes` trait is used to read bytes from the wire.
// Many of the methods have an `allow(dead_code)` attribute because they are not currently used but they could be anytime in the future.
pub(crate) trait SqlReadBytes: AsyncRead + Unpin {
    // Pretty-print current wire content.
    #[allow(dead_code)]
    fn debug_buffer(&self);

    // The client state.
    fn context(&self) -> &Context;

    // A mutable reference to the SQL client state.
    fn context_mut(&mut self) -> &mut Context;

    // Read a single i8 value.
    #[allow(dead_code)]
    fn read_i8(&mut self) -> ReadI8<&mut Self>
    where
        Self: Unpin,
    {
        ReadI8::new(self)
    }

    // Read a single byte value.
    fn read_u8(&mut self) -> ReadU8<&mut Self>
    where
        Self: Unpin,
    {
        ReadU8::new(self)
    }

    // Read a single big-endian u32 value. (LOGINACK, its only user, now
    // parses from a bounded `TokenBody` instead.)
    #[allow(dead_code)]
    fn read_u32(&mut self) -> ReadU32Be<&mut Self>
    where
        Self: Unpin,
    {
        ReadU32Be::new(self)
    }

    // Read a single big-endian f32 value.
    #[allow(dead_code)]
    fn read_f32(&mut self) -> ReadF32<&mut Self>
    where
        Self: Unpin,
    {
        ReadF32::new(self)
    }

    // Read a single big-endian f64 value.
    #[allow(dead_code)]
    fn read_f64(&mut self) -> ReadF64<&mut Self>
    where
        Self: Unpin,
    {
        ReadF64::new(self)
    }

    // Read a single f32 value.
    fn read_f32_le(&mut self) -> ReadF32Le<&mut Self>
    where
        Self: Unpin,
    {
        ReadF32Le::new(self)
    }

    // Read a single f64 value.
    fn read_f64_le(&mut self) -> ReadF64Le<&mut Self>
    where
        Self: Unpin,
    {
        ReadF64Le::new(self)
    }

    // Read a single u16 value.
    fn read_u16_le(&mut self) -> ReadU16Le<&mut Self>
    where
        Self: Unpin,
    {
        ReadU16Le::new(self)
    }

    // Read a single u32 value.
    fn read_u32_le(&mut self) -> ReadU32Le<&mut Self>
    where
        Self: Unpin,
    {
        ReadU32Le::new(self)
    }

    // Read a single u64 value.
    fn read_u64_le(&mut self) -> ReadU64Le<&mut Self>
    where
        Self: Unpin,
    {
        ReadU64Le::new(self)
    }

    // Read a single u128 value.
    #[allow(dead_code)]
    fn read_u128_le(&mut self) -> ReadU128Le<&mut Self>
    where
        Self: Unpin,
    {
        ReadU128Le::new(self)
    }

    // Read a single i16 value.
    fn read_i16_le(&mut self) -> ReadI16Le<&mut Self>
    where
        Self: Unpin,
    {
        ReadI16Le::new(self)
    }

    // Read a single i32 value.
    fn read_i32_le(&mut self) -> ReadI32Le<&mut Self>
    where
        Self: Unpin,
    {
        ReadI32Le::new(self)
    }

    // Read a single i64 value.
    fn read_i64_le(&mut self) -> ReadI64Le<&mut Self>
    where
        Self: Unpin,
    {
        ReadI64Le::new(self)
    }

    // Read a single i128 value.
    #[allow(dead_code)]
    fn read_i128_le(&mut self) -> ReadI128Le<&mut Self>
    where
        Self: Unpin,
    {
        ReadI128Le::new(self)
    }

    // A variable-length character stream defined by a length-field of an u8.
    fn read_b_varchar(&mut self) -> ReadBVarchar<&mut Self>
    where
        Self: Unpin,
    {
        ReadBVarchar::new(self)
    }

    // A variable-length character stream defined by a length-field of an u16.
    fn read_us_varchar(&mut self) -> ReadUSVarchar<&mut Self>
    where
        Self: Unpin,
    {
        ReadUSVarchar::new(self)
    }
}

varchar_reader!(ReadBVarchar, ReadU8);
varchar_reader!(ReadUSVarchar, ReadU16Le);

bytes_reader!(ReadI8, i8, get_i8);
bytes_reader!(ReadU8, u8, get_u8);
bytes_reader!(ReadU32Be, u32, get_u32);

bytes_reader!(ReadU16Le, u16, get_u16_le);
bytes_reader!(ReadU32Le, u32, get_u32_le);
bytes_reader!(ReadU64Le, u64, get_u64_le);
bytes_reader!(ReadU128Le, u128, get_u128_le);

bytes_reader!(ReadI16Le, i16, get_i16_le);
bytes_reader!(ReadI32Le, i32, get_i32_le);
bytes_reader!(ReadI64Le, i64, get_i64_le);
bytes_reader!(ReadI128Le, i128, get_i128_le);

bytes_reader!(ReadF32, f32, get_f32);
bytes_reader!(ReadF64, f64, get_f64);

bytes_reader!(ReadF32Le, f32, get_f32_le);
bytes_reader!(ReadF64Le, f64, get_f64_le);

#[cfg(test)]
pub(crate) mod test_utils {
    use crate::tds::Context;
    use crate::SqlReadBytes;
    use bytes::BytesMut;
    use futures_util::io::AsyncRead;
    use std::io;
    use std::pin::Pin;
    use std::task::Poll;

    // a test util to run decode logic on BytesMut, for testing loop back
    pub(crate) trait IntoSqlReadBytes {
        type T: SqlReadBytes;
        fn into_sql_read_bytes(self) -> Self::T;
    }

    impl IntoSqlReadBytes for BytesMut {
        type T = BytesMutReader;

        fn into_sql_read_bytes(self) -> Self::T {
            BytesMutReader {
                buf: self,
                context: Context::new(),
            }
        }
    }

    pub(crate) struct BytesMutReader {
        buf: BytesMut,
        // A default context, so decoders that consult it (e.g. ERROR's
        // version-dependent line width) can be unit-tested.
        context: Context,
    }

    impl AsyncRead for BytesMutReader {
        fn poll_read(
            self: Pin<&mut Self>,
            _cx: &mut std::task::Context<'_>,
            buf: &mut [u8],
        ) -> Poll<std::io::Result<usize>> {
            let this = self.get_mut();
            let size = buf.len();

            // Got EOF before having all the data.
            if this.buf.len() < size {
                return Poll::Ready(Err(io::Error::new(
                    io::ErrorKind::UnexpectedEof,
                    "No more packets in the wire",
                )));
            }

            buf.copy_from_slice(this.buf.split_to(size).as_ref());
            Poll::Ready(Ok(size))
        }
    }

    impl SqlReadBytes for BytesMutReader {
        fn debug_buffer(&self) {
            todo!()
        }

        fn context(&self) -> &Context {
            &self.context
        }

        fn context_mut(&mut self) -> &mut Context {
            &mut self.context
        }
    }
}

#[cfg(test)]
mod reader_tests {
    use super::test_utils::IntoSqlReadBytes;
    use super::*;
    use bytes::BytesMut;

    #[tokio::test]
    async fn every_primitive_reader_handles_endianness() {
        let mut reader = BytesMut::from(
            &[
                0xff, // i8
                0x01, 0x02, 0x03, 0x04, // u32 be
                0x3f, 0x80, 0x00, 0x00, // f32 be
                0x3f, 0xf0, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, // f64 be
            ][..],
        )
        .into_sql_read_bytes();
        assert_eq!(reader.read_i8().await.unwrap(), -1);
        assert_eq!(reader.read_u32().await.unwrap(), 0x0102_0304);
        assert_eq!(reader.read_f32().await.unwrap(), 1.0);
        assert_eq!(reader.read_f64().await.unwrap(), 1.0);

        let mut reader = BytesMut::from(&u128::MAX.to_le_bytes()[..]).into_sql_read_bytes();
        assert_eq!(reader.read_u128_le().await.unwrap(), u128::MAX);
        let mut reader = BytesMut::from(&i128::MIN.to_le_bytes()[..]).into_sql_read_bytes();
        assert_eq!(reader.read_i128_le().await.unwrap(), i128::MIN);
    }

    #[tokio::test]
    async fn varchar_readers_cover_empty_invalid_and_truncated_input() {
        let mut empty = BytesMut::from(&[0][..]).into_sql_read_bytes();
        assert_eq!(empty.read_b_varchar().await.unwrap(), "");

        let mut invalid = BytesMut::from(&[1, 0, 0, 0xd8][..]).into_sql_read_bytes();
        assert_eq!(
            invalid.read_us_varchar().await.unwrap_err().kind(),
            io::ErrorKind::InvalidData
        );

        let mut truncated = BytesMut::from(&[2, b'a', 0][..]).into_sql_read_bytes();
        assert_eq!(
            truncated.read_b_varchar().await.unwrap_err().kind(),
            io::ErrorKind::UnexpectedEof
        );
    }

    #[tokio::test]
    async fn bounded_byte_reads_rollback_partial_output_on_eof() {
        let mut source = futures_util::io::Cursor::new(vec![1, 2]);
        let mut output = vec![9];
        let error = read_bytes_into(&mut source, &mut output, 3)
            .await
            .unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::UnexpectedEof);
        assert_eq!(output, vec![9]);

        let input = vec![7; MAX_PREALLOC + 1];
        let mut source = futures_util::io::Cursor::new(input.clone());
        let mut output = Vec::new();
        read_bytes_into(&mut source, &mut output, input.len())
            .await
            .unwrap();
        assert_eq!(output, input);
    }

    /// Returns `Pending` before every read, then serves whole reads, and
    /// then either a clean end of stream or an I/O error.
    struct Stuttering {
        data: std::collections::VecDeque<u8>,
        pending: bool,
        fail_at_end: bool,
    }

    impl Stuttering {
        fn new(data: &[u8], fail_at_end: bool) -> Self {
            Self {
                data: data.iter().copied().collect(),
                pending: true,
                fail_at_end,
            }
        }
    }

    impl AsyncRead for Stuttering {
        fn poll_read(
            mut self: Pin<&mut Self>,
            cx: &mut task::Context<'_>,
            buf: &mut [u8],
        ) -> Poll<io::Result<usize>> {
            let pending = self.pending;
            self.pending = !pending;
            if pending {
                cx.waker().wake_by_ref();
                return Poll::Pending;
            }
            // Whole reads only: the field readers restart a partly read value
            // after `Pending`, so a short read would drop its first bytes.
            if self.data.len() >= buf.len() {
                for slot in buf.iter_mut() {
                    *slot = self.data.pop_front().unwrap_or_default();
                }
                return Poll::Ready(Ok(buf.len()));
            }
            match self.data.pop_front() {
                Some(byte) => {
                    buf[0] = byte;
                    Poll::Ready(Ok(1))
                }
                None if self.fail_at_end => {
                    Poll::Ready(Err(io::Error::new(io::ErrorKind::BrokenPipe, "gone")))
                }
                None => Poll::Ready(Ok(0)),
            }
        }
    }

    #[tokio::test]
    async fn readers_resume_after_pending_and_report_errors_and_eof() {
        assert_eq!(
            ReadBVarchar::new(Stuttering::new(&[2, b'h', 0, b'i', 0], false))
                .await
                .unwrap(),
            "hi"
        );
        assert_eq!(
            ReadUSVarchar::new(Stuttering::new(&[1, 0, b'h', 0], false))
                .await
                .unwrap(),
            "h"
        );
        assert_eq!(
            ReadU32Le::new(Stuttering::new(&[1, 0, 0, 0], false))
                .await
                .unwrap(),
            1
        );

        // The length prefix and the body each fail on a broken transport.
        let error = ReadBVarchar::new(Stuttering::new(&[], true))
            .await
            .unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::BrokenPipe);
        let error = ReadBVarchar::new(Stuttering::new(&[1], true))
            .await
            .unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::BrokenPipe);
        let error = ReadU32Le::new(Stuttering::new(&[1], true))
            .await
            .unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::BrokenPipe);

        // A clean end of stream before the value is complete.
        let error = ReadU32Le::new(Stuttering::new(&[1], false))
            .await
            .unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::UnexpectedEof);
    }

    #[tokio::test]
    async fn a_completed_field_reader_can_be_polled_again() {
        let mut reader = Box::pin(ReadU16Le::new(futures_util::io::Cursor::new(vec![1, 2])));
        assert_eq!(reader.as_mut().await.unwrap(), 0x0201);
        assert_eq!(reader.as_mut().await.unwrap(), 0x0201);
    }
}
