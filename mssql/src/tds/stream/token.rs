use crate::tds::codec::TokenSspi;
use crate::{
    client::Connection,
    tds::codec::{
        TokenColMetaData, TokenDone, TokenEnvChange, TokenError, TokenFeatureExtAck, TokenInfo,
        TokenLoginAck, TokenOrder, TokenReturnValue, TokenRow,
    },
    Error, SqlReadBytes, TokenType,
};
use futures_util::{
    io::{AsyncRead, AsyncWrite},
    stream::{BoxStream, Stream, StreamExt, TryStreamExt},
};
use std::future::Future;
use std::pin::Pin;
use std::sync::atomic::{AtomicBool, Ordering};
use std::task::{Context as TaskContext, Poll};
use std::time::Duration;
use std::{convert::TryFrom, sync::Arc};
use tracing::{event, Level};

#[derive(Debug)]
#[allow(dead_code)]
pub enum ReceivedToken {
    NewResultset(Arc<TokenColMetaData<'static>>),
    Row(TokenRow<'static>),
    Done(TokenDone),
    DoneInProc(TokenDone),
    DoneProc(TokenDone),
    ReturnStatus(u32),
    ReturnValue(TokenReturnValue),
    Order(TokenOrder),
    EnvChange(TokenEnvChange),
    Info(TokenInfo),
    LoginAck(TokenLoginAck),
    Sspi(TokenSspi),
    FeatureExtAck(TokenFeatureExtAck),
    Error(TokenError),
}

pub(crate) struct TokenStream<'a, S: AsyncRead + AsyncWrite + Unpin + Send> {
    conn: &'a mut Connection<S>,
    last_error: Option<Error>,
}

impl<'a, S> TokenStream<'a, S>
where
    S: AsyncRead + AsyncWrite + Unpin + Send,
{
    pub(crate) fn new(conn: &'a mut Connection<S>) -> Self {
        Self {
            conn,
            last_error: None,
        }
    }

    pub(crate) async fn flush_done(self) -> crate::Result<TokenDone> {
        let mut stream = self.try_unfold();
        let mut last_error = None;
        let mut routing = None;

        loop {
            match stream.try_next().await? {
                Some(ReceivedToken::Error(error)) => {
                    if last_error.is_none() {
                        last_error = Some(error);
                    }
                }
                Some(ReceivedToken::Done(token)) => match (last_error, routing) {
                    (Some(error), _) => return Err(Error::Server(error)),
                    (_, Some(routing)) => return Err(routing),
                    (_, _) => return Ok(token),
                },
                Some(ReceivedToken::EnvChange(TokenEnvChange::Routing { host, port })) => {
                    routing = Some(Error::Routing { host, port });
                }
                Some(_) => (),
                None => return Err(crate::Error::Protocol("Never got DONE token.".into())),
            }
        }
    }

    #[cfg(any(windows, feature = "integrated-auth-gssapi", feature = "sspi-rs"))]
    pub(crate) async fn flush_sspi(self) -> crate::Result<TokenSspi> {
        let mut stream = self.try_unfold();
        let mut last_error = None;

        loop {
            match stream.try_next().await? {
                Some(ReceivedToken::Error(error)) => {
                    if last_error.is_none() {
                        last_error = Some(error);
                    }
                }
                Some(ReceivedToken::Sspi(token)) => return Ok(token),
                Some(_) => (),
                None => match last_error {
                    Some(err) => return Err(crate::Error::Server(err)),
                    None => return Err(crate::Error::Protocol("Never got SSPI token.".into())),
                },
            }
        }
    }

    async fn get_col_metadata(&mut self) -> crate::Result<ReceivedToken> {
        let meta = Arc::new(TokenColMetaData::decode(self.conn).await?);
        self.conn.context_mut().set_last_meta(meta.clone());

        event!(Level::TRACE, ?meta);

        Ok(ReceivedToken::NewResultset(meta))
    }

    async fn get_row(&mut self) -> crate::Result<ReceivedToken> {
        let return_value = TokenRow::decode(self.conn).await?;

        event!(Level::TRACE, message = ?return_value);
        Ok(ReceivedToken::Row(return_value))
    }

    async fn get_nbc_row(&mut self) -> crate::Result<ReceivedToken> {
        let return_value = TokenRow::decode_nbc(self.conn).await?;

        event!(Level::TRACE, message = ?return_value);
        Ok(ReceivedToken::Row(return_value))
    }

    async fn get_return_value(&mut self) -> crate::Result<ReceivedToken> {
        let return_value = TokenReturnValue::decode(self.conn).await?;
        event!(Level::TRACE, message = ?return_value);
        Ok(ReceivedToken::ReturnValue(return_value))
    }

    async fn get_return_status(&mut self) -> crate::Result<ReceivedToken> {
        let status = self.conn.read_u32_le().await?;
        Ok(ReceivedToken::ReturnStatus(status))
    }

    async fn get_error(&mut self) -> crate::Result<ReceivedToken> {
        let err = TokenError::decode(self.conn).await?;

        if self.last_error.is_none() {
            self.last_error = Some(Error::Server(err.clone()));
        }

        event!(Level::ERROR, message = %err.message, code = err.code);
        Ok(ReceivedToken::Error(err))
    }

    async fn get_order(&mut self) -> crate::Result<ReceivedToken> {
        let order = TokenOrder::decode(self.conn).await?;
        event!(Level::TRACE, message = ?order);
        Ok(ReceivedToken::Order(order))
    }

    async fn get_done_value(&mut self) -> crate::Result<ReceivedToken> {
        let done = TokenDone::decode(self.conn).await?;
        event!(Level::TRACE, "{}", done);
        Ok(ReceivedToken::Done(done))
    }

    async fn get_done_proc_value(&mut self) -> crate::Result<ReceivedToken> {
        let done = TokenDone::decode(self.conn).await?;
        event!(Level::TRACE, "{}", done);
        Ok(ReceivedToken::DoneProc(done))
    }

    async fn get_done_in_proc_value(&mut self) -> crate::Result<ReceivedToken> {
        let done = TokenDone::decode(self.conn).await?;
        event!(Level::TRACE, "{}", done);
        Ok(ReceivedToken::DoneInProc(done))
    }

    async fn get_env_change(&mut self) -> crate::Result<ReceivedToken> {
        let change = TokenEnvChange::decode(self.conn).await?;

        match change {
            TokenEnvChange::PacketSize(new_size, _) => {
                let new_size = validate_negotiated_packet_size(new_size)?;
                self.conn.context_mut().set_packet_size(new_size);
            }
            TokenEnvChange::BeginTransaction(desc) => {
                self.conn.context_mut().set_transaction_descriptor(desc);
            }
            TokenEnvChange::CommitTransaction
            | TokenEnvChange::RollbackTransaction
            | TokenEnvChange::DefectTransaction => {
                self.conn.context_mut().set_transaction_descriptor([0; 8]);
            }
            _ => (),
        }

        event!(Level::DEBUG, "{}", change);

        Ok(ReceivedToken::EnvChange(change))
    }

    async fn get_info(&mut self) -> crate::Result<ReceivedToken> {
        let info = TokenInfo::decode(self.conn).await?;
        event!(Level::DEBUG, "{}", info.message);
        Ok(ReceivedToken::Info(info))
    }

    async fn get_login_ack(&mut self) -> crate::Result<ReceivedToken> {
        let ack = TokenLoginAck::decode(self.conn).await?;
        event!(Level::DEBUG, "{} version {}", ack.prog_name, ack.version);
        Ok(ReceivedToken::LoginAck(ack))
    }

    async fn get_feature_ext_ack(&mut self) -> crate::Result<ReceivedToken> {
        let ack = TokenFeatureExtAck::decode(self.conn).await?;
        event!(
            Level::DEBUG,
            "FeatureExtAck with {} features",
            ack.features.len()
        );
        Ok(ReceivedToken::FeatureExtAck(ack))
    }

    async fn get_sspi(&mut self) -> crate::Result<ReceivedToken> {
        let sspi = TokenSspi::decode_async(self.conn).await?;
        event!(Level::TRACE, "SSPI response");
        Ok(ReceivedToken::Sspi(sspi))
    }

    pub fn try_unfold(self) -> BoxStream<'a, crate::Result<ReceivedToken>> {
        // Every command-result read goes through here (query/execute/
        // simple_query streams, the bulk-insert acknowledgement,
        // column_metadata), so this one wrapper bounds them all. The context
        // value is `None` until login completes.
        let command_timeout = self.conn.context().command_timeout();
        let desync = self.conn.command_desync_flag();

        let stream = futures_util::stream::try_unfold(self, |mut this| async move {
            let ty = loop {
                if this.conn.is_eof() {
                    match this.last_error {
                        None => return Ok(None),
                        Some(error) => return Err(error),
                    }
                }

                let ty_byte = this.conn.read_u8().await?;

                let ty = TokenType::try_from(ty_byte).map_err(|_| {
                    Error::Protocol(format!("invalid token type {:x}", ty_byte).into())
                })?;

                match ty {
                    // Browse-mode metadata (`FOR BROWSE`, or a cursor/
                    // `NO_BROWSETABLE` session) that this client has no use
                    // for. Both are u16-length-prefixed, so skip them by
                    // length instead of failing the whole result with
                    // "unhandled token type". (Upstream Tiberius 8b6e080 and
                    // 617dc43 decode them; that isn't needed here.)
                    TokenType::ColInfo | TokenType::TabName => {
                        let len = usize::from(this.conn.read_u16_le().await?);
                        let mut skipped = vec![0u8; len];
                        futures_util::io::AsyncReadExt::read_exact(&mut this.conn, &mut skipped)
                            .await?;
                        event!(Level::TRACE, "skipped {:?} token ({} bytes)", ty, len);
                    }
                    ty => break ty,
                }
            };

            let token = match ty {
                TokenType::ReturnStatus => this.get_return_status().await?,
                TokenType::ColMetaData => this.get_col_metadata().await?,
                TokenType::Row => this.get_row().await?,
                TokenType::NbcRow => this.get_nbc_row().await?,
                TokenType::Done => this.get_done_value().await?,
                TokenType::DoneProc => this.get_done_proc_value().await?,
                TokenType::DoneInProc => this.get_done_in_proc_value().await?,
                TokenType::ReturnValue => this.get_return_value().await?,
                TokenType::Error => this.get_error().await?,
                TokenType::Order => this.get_order().await?,
                TokenType::EnvChange => this.get_env_change().await?,
                TokenType::Info => this.get_info().await?,
                TokenType::LoginAck => this.get_login_ack().await?,
                TokenType::Sspi => this.get_sspi().await?,
                TokenType::FeatureExtAck => this.get_feature_ext_ack().await?,
                _ => {
                    return Err(Error::Protocol(
                        format!("unhandled token type {:?}", ty).into(),
                    ))
                }
            };

            Ok(Some((token, this)))
        });

        Box::pin(RoundTripTimeout::new(
            command_timeout,
            Some(desync),
            Box::pin(stream),
        ))
    }
}

/// Bounds each server round-trip of a token stream (`Config::command_timeout`).
/// The timer runs only while the inner stream is `Pending` waiting on the
/// server, and resets whenever a token is delivered, so a slow consumer never
/// trips it; only a stalled server does. When it fires, the connection's
/// shared desync flag is set (the rest of the response is still unread) and
/// the stream yields an `Io` `TimedOut` error. Ported from upstream Tiberius
/// (tiberius-rs/tiberius 2c63330).
struct RoundTripTimeout<'a> {
    inner: BoxStream<'a, crate::Result<ReceivedToken>>,
    timeout: Option<Duration>,
    delay: Option<futures_timer::Delay>,
    desync: Option<Arc<AtomicBool>>,
}

impl<'a> RoundTripTimeout<'a> {
    fn new(
        timeout: Option<Duration>,
        desync: Option<Arc<AtomicBool>>,
        inner: BoxStream<'a, crate::Result<ReceivedToken>>,
    ) -> Self {
        Self {
            inner,
            timeout,
            delay: None,
            desync,
        }
    }
}

impl Stream for RoundTripTimeout<'_> {
    type Item = crate::Result<ReceivedToken>;

    fn poll_next(self: Pin<&mut Self>, cx: &mut TaskContext<'_>) -> Poll<Option<Self::Item>> {
        let this = self.get_mut();

        match this.inner.poll_next_unpin(cx) {
            Poll::Ready(item) => {
                this.delay = None;
                Poll::Ready(item)
            }
            Poll::Pending => {
                let Some(timeout) = this.timeout else {
                    return Poll::Pending;
                };

                let delay = this
                    .delay
                    .get_or_insert_with(|| futures_timer::Delay::new(timeout));

                match Pin::new(delay).poll(cx) {
                    Poll::Ready(()) => {
                        this.delay = None;
                        if let Some(desync) = &this.desync {
                            desync.store(true, Ordering::Release);
                        }
                        Poll::Ready(Some(Err(Error::Io {
                            kind: std::io::ErrorKind::TimedOut,
                            message: format!(
                                "the server sent nothing more of the command result within \
                                 {timeout:?}; the connection is now out of sync and must be \
                                 dropped. Raise the bound with `Config::command_timeout`, or \
                                 pass `None` to wait indefinitely."
                            ),
                        })))
                    }
                    Poll::Pending => Poll::Pending,
                }
            }
        }
    }
}

#[cfg(test)]
mod round_trip_timeout_tests {
    use super::*;
    use futures_util::stream;

    fn done() -> crate::Result<ReceivedToken> {
        Ok(ReceivedToken::Done(TokenDone::default()))
    }

    #[tokio::test]
    async fn a_stalled_server_times_out_and_marks_desync() {
        let desync = Arc::new(AtomicBool::new(false));
        let inner = stream::iter(vec![done()]).chain(stream::pending()).boxed();
        let mut s =
            RoundTripTimeout::new(Some(Duration::from_millis(30)), Some(desync.clone()), inner);

        assert!(s.next().await.unwrap().is_ok());
        let err = s.next().await.unwrap().unwrap_err();
        assert!(
            matches!(
                err,
                Error::Io {
                    kind: std::io::ErrorKind::TimedOut,
                    ..
                }
            ),
            "{err:?}"
        );
        assert!(desync.load(Ordering::Acquire));
    }

    #[tokio::test]
    async fn slow_consumer_does_not_trip_it() {
        let inner = stream::iter(vec![done(), done()]).boxed();
        let mut s = RoundTripTimeout::new(Some(Duration::from_millis(20)), None, inner);
        assert!(s.next().await.unwrap().is_ok());
        tokio::time::sleep(Duration::from_millis(60)).await;
        assert!(s.next().await.unwrap().is_ok());
        assert!(s.next().await.is_none());
    }

    #[tokio::test]
    async fn none_is_unbounded() {
        let inner = stream::pending::<crate::Result<ReceivedToken>>().boxed();
        let mut s = RoundTripTimeout::new(None, None, inner);
        let polled = tokio::time::timeout(Duration::from_millis(50), s.next()).await;
        assert!(polled.is_err(), "must still be pending");
    }
}

/// MS-TDS: a negotiated packet size must be within 512..=32767. An
/// out-of-range ENVCHANGE value from the server used to be stored as-is:
/// exactly 8 made the send path's `packet_size - HEADER_BYTES` chunk size
/// zero and spun `split_to(0)` forever, below 8 underflowed, and above 65535
/// truncated the u16 frame length and desynced the stream. Ported from
/// upstream Tiberius (tiberius-rs/tiberius bf96855 / 3e6afc3).
fn validate_negotiated_packet_size(new_size: u32) -> crate::Result<u32> {
    if (512..=32767).contains(&new_size) {
        Ok(new_size)
    } else {
        Err(crate::Error::Protocol(
            format!("server requested an invalid packet size of {new_size}").into(),
        ))
    }
}

#[cfg(test)]
mod packet_size_tests {
    use super::validate_negotiated_packet_size;

    #[test]
    fn out_of_range_negotiated_packet_size_is_rejected() {
        for size in [0, 7, 8, 511, 32768, 65536, u32::MAX] {
            let err = validate_negotiated_packet_size(size).unwrap_err();
            assert!(matches!(err, crate::Error::Protocol(_)), "{size}: {err:?}");
        }
    }

    #[test]
    fn in_range_negotiated_packet_size_is_accepted() {
        for size in [512, 4096, 8000, 32767] {
            assert_eq!(validate_negotiated_packet_size(size).unwrap(), size);
        }
    }
}
