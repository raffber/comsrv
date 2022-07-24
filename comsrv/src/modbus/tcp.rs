use crate::modbus::{FunctionCode, ModBusException, TransactionInfo};
use anyhow::anyhow;
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};

pub struct TcpHandler<T: FunctionCode> {
    function_code: T,
}

impl<T: FunctionCode> TcpHandler<T> {
    pub fn new(function_code: T) -> Self {
        Self { function_code }
    }

    pub async fn handle<S: AsyncRead + AsyncWrite + Unpin>(
        &self,
        transaction: &TransactionInfo,
        stream: &mut S,
    ) -> crate::Result<T::Output> {
        let mut request = Vec::new();
        request.extend(&transaction.transaction_id.to_be_bytes());
        request.extend(&[0u8, 0, 0, 0]);
        request.extend(&[transaction.station_address, self.function_code.function_code()]);
        self.function_code.format_request(&mut request);
        let mut l = request.len();
        l -= 6;
        if l > u16::MAX as usize {
            return Err(crate::Error::argument(anyhow!("ModBus frame over length.")));
        }
        let len_buf = (l as u16).to_be_bytes();
        request[4] = len_buf[0];
        request[5] = len_buf[1];
        stream.write_all(&request).await.map_err(crate::Error::transport)?;
        let reply = read_tcp_frame(&transaction, self.function_code.function_code(), stream).await?;
        let header_len = self.function_code.get_header_length();
        if reply.len() < header_len {
            return Err(crate::Error::argument(anyhow!("ModBus frame shorter than header")));
        }
        let data_len = self.function_code.get_data_length_from_header(&reply[0..header_len])?;
        if reply.len() - header_len < data_len {
            return Err(crate::Error::argument(anyhow!("ModBus frame data part shorter than expected")));
        }
        self.function_code.parse_frame(&reply)
    }
}

async fn read_tcp_frame<T: AsyncRead + AsyncWrite + Unpin>(
    transaction: &TransactionInfo,
    function_code: u8,
    stream: &mut T,
) -> crate::Result<Vec<u8>> {
    let mut header = [0_u8; 8];
    stream.read_exact(&mut header).await.map_err(crate::Error::transport)?;
    let transaction_id = u16::from_be_bytes([header[0], header[1]]);
    let proto = u16::from_be_bytes([header[2], header[3]]);
    let len = u16::from_be_bytes([header[4], header[5]]);
    let station_address = header[6];
    let parsed_function_code = header[7];
    if station_address != transaction.station_address
        || transaction_id != transaction.transaction_id
        || proto != 0
        || len < 2
    {
        return Err(crate::Error::protocol(anyhow!("Invalid answer received.")));
    }
    if parsed_function_code == (0x80 | function_code) {
        let exception_code = stream.read_u8().await?;
        return Err(crate::Error::protocol(anyhow!(ModBusException::from_code(exception_code))));
    } else if parsed_function_code != function_code {
        return Err(crate::Error::protocol(anyhow!("Invalid frame")));
    }
    let mut buf = vec![0_u8; (len - 2) as usize];
    stream.read_exact(&mut buf).await.map_err(crate::Error::transport)?;
    Ok(buf)
}
