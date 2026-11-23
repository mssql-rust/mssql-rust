use asynchronous_codec::BytesMut;
use futures_util::io::{AsyncRead, AsyncWrite};
use tracing::{event, Level};

use crate::{
    client::Connection, sql_read_bytes::SqlReadBytes, BytesMutWithDataColumns, ExecuteResult,
};

use super::{
    Encode, MetaDataColumn, PacketHeader, PacketStatus, TokenDone, TokenRow, HEADER_BYTES,
};

/// A handler for a bulk insert data flow.
#[derive(Debug)]
pub struct BulkLoadRequest<'a, S>
where
    S: AsyncRead + AsyncWrite + Unpin + Send,
{
    connection: &'a mut Connection<S>,
    packet_id: u8,
    buf: BytesMut,
    columns: Vec<MetaDataColumn<'a>>,
}

impl<'a, S> BulkLoadRequest<'a, S>
where
    S: AsyncRead + AsyncWrite + Unpin + Send,
{
    pub(crate) fn new(
        connection: &'a mut Connection<S>,
        columns: Vec<MetaDataColumn<'a>>,
        table_name: &str,
    ) -> crate::Result<Self> {
        let packet_id = connection.context_mut().next_packet_id();
        let mut buf = BytesMut::new();

        super::token::encode_bulk_col_metadata(&columns, table_name, &mut buf)?;

        let this = Self {
            connection,
            packet_id,
            buf,
            columns,
        };

        Ok(this)
    }

    /// Adds a new row to the bulk insert, flushing only when having a full packet of data.
    ///
    /// # Warning
    ///
    /// After the last row, [`finalize`] must be called to flush the buffered
    /// data and for the data to actually be available in the table.
    ///
    /// [`finalize`]: #method.finalize
    pub async fn send(&mut self, row: TokenRow<'a>) -> crate::Result<()> {
        // A row that fails to encode partway (e.g. a smallmoney out of range,
        // or a Numeric that can't be rescaled) used to leave its partial
        // bytes in the buffer, to be flushed later and desync the bulk-load
        // stream. Roll back to the row boundary instead, so the caller can
        // skip the bad row and carry on. Ported from upstream Tiberius 991702f.
        // A value whose ToSql/IntoSql conversion failed is rejected before
        // any of the row is encoded.
        super::reject_invalid_values(row.iter())?;

        let row_start = self.buf.len();
        let mut buf_with_columns = BytesMutWithDataColumns::new(&mut self.buf, &self.columns);

        if let Err(e) = row.encode(&mut buf_with_columns) {
            self.buf.truncate(row_start);
            return Err(e);
        }

        self.write_packets().await?;

        Ok(())
    }

    /// Ends the bulk load, flushing all pending data to the wire.
    ///
    /// This method must be called after sending all the data to flush all
    /// pending data and to get the server actually to store the rows to the
    /// table.
    pub async fn finalize(mut self) -> crate::Result<ExecuteResult> {
        TokenDone::default().encode(&mut self.buf)?;
        self.write_packets().await?;

        let mut header = PacketHeader::bulk_load(self.packet_id);
        header.set_status(PacketStatus::EndOfMessage);

        let data = self.buf.split();

        event!(
            Level::TRACE,
            "Finalizing a bulk insert ({} bytes)",
            data.len() + HEADER_BYTES,
        );

        // Mark the connection only while a write is in flight. Dropping a
        // `BulkLoadRequest` between rows is recoverable (verified live: the
        // server ends the abandoned load on the next request), but dropping
        // it mid-write leaves a torn packet.
        self.connection.poison();
        self.connection.write_to_wire(header, data).await?;
        self.connection.flush_sink().await?;
        self.connection.unpoison();

        ExecuteResult::new(self.connection).await
    }

    async fn write_packets(&mut self) -> crate::Result<()> {
        let packet_size = self.connection.context().payload_size()?;

        if self.buf.len() <= packet_size {
            return Ok(());
        }

        self.connection.poison();

        while self.buf.len() > packet_size {
            let header = PacketHeader::bulk_load(self.packet_id);
            let data = self.buf.split_to(packet_size);

            event!(
                Level::TRACE,
                "Bulk insert packet ({} bytes)",
                data.len() + HEADER_BYTES,
            );

            self.connection.write_to_wire(header, data).await?;
        }

        // `write_to_wire` hands each packet to the framed sink; flush so a
        // packet can't be left half-sent in the sink's buffer once the flag
        // is cleared.
        self.connection.flush_sink().await?;
        self.connection.unpoison();

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tds::codec::ColumnData;

    #[test]
    fn a_row_with_an_invalid_value_is_rejected_before_any_encoding() {
        let rt = tokio::runtime::Builder::new_current_thread()
            .build()
            .unwrap();
        rt.block_on(async {
            let mut connection = Connection::in_memory();
            let mut request = BulkLoadRequest::new(&mut connection, Vec::new(), "t").unwrap();
            let buffered = request.buf.len();

            let mut row = TokenRow::new();
            row.push(ColumnData::I32(Some(1)));
            row.push(ColumnData::Invalid("date out of range".into()));
            let err = request.send(row).await.unwrap_err();

            assert!(matches!(err, crate::Error::Conversion(m) if m == "date out of range"));
            assert_eq!(request.buf.len(), buffered);
            drop(request);
            assert_eq!(connection.bytes_written(), 0);
        });
    }
}
