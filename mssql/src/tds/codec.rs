mod batch_request;
mod bulk_load;
mod column_data;
mod decode;
mod encode;
mod guid;
mod header;
mod iterator_ext;
mod login;
mod packet;
mod pre_login;
mod rpc_request;
mod token;
mod transaction_manager;
mod tvp;
mod type_info;

pub use batch_request::*;
pub use bulk_load::*;
use bytes::BytesMut;
pub use column_data::*;
pub use decode::*;
pub(crate) use encode::*;
use futures_util::stream::{Stream, TryStreamExt};
pub use header::*;
pub(crate) use iterator_ext::*;
pub use login::*;
pub use packet::*;
pub use pre_login::*;
pub use rpc_request::*;
pub use token::*;
pub use transaction_manager::IsolationLevel;
pub(crate) use transaction_manager::TransactionManagerRequest;
pub(crate) use tvp::{tvp_column_type, TableValue};
pub use type_info::*;

const HEADER_BYTES: usize = 8;
/// Total length of the transaction-descriptor ALL_HEADERS block.
const ALL_HEADERS_LEN_TX: u32 = 22;
/// The same block's length without its own 4-byte `TotalLength` field.
const ALL_HEADERS_LEN_TX_HEADER: u32 = ALL_HEADERS_LEN_TX - 4;

#[derive(Debug)]
#[repr(u16)]
#[allow(dead_code)]
enum AllHeaderTy {
    QueryDescriptor = 1,
    TransactionDescriptor = 2,
    TraceActivity = 3,
}

enum_into_int!(AllHeaderTy => u16);

pub struct PacketCodec;

pub(crate) async fn collect_from<S, T>(stream: &mut S) -> crate::Result<T>
where
    T: Decode<BytesMut> + Sized,
    S: Stream<Item = crate::Result<Packet>> + Unpin,
{
    let mut buf = BytesMut::new();

    while let Some(packet) = stream.try_next().await? {
        let is_last = packet.is_last();
        let (_, payload) = packet.into_parts();
        buf.extend_from_slice(&payload);

        if is_last {
            break;
        }
    }

    T::decode(&mut buf)
}
