use super::{FunctionCode, ModBusException, TransactionInfo};
use anyhow::anyhow;
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};

pub struct RtuHandler<T: FunctionCode> {
    function_code: T,
}

impl<T: FunctionCode> RtuHandler<T> {
    pub fn new(function_code: T) -> Self {
        Self { function_code }
    }

    pub async fn handle<S: AsyncRead + AsyncWrite + Unpin>(
        &self,
        transaction: &TransactionInfo,
        stream: &mut S,
    ) -> crate::Result<T::Output> {
        let mut request = Vec::new();
        request.extend(&[transaction.station_address, self.function_code.function_code()]);
        self.function_code.format_request(&mut request);
        let l = request.len();
        if l > u8::MAX as usize {
            return Err(crate::Error::argument(anyhow!("ModBus frame over length.")));
        }
        request.extend(&crc(&request).to_le_bytes());
        stream.write_all(&request).await.map_err(crate::Error::transport)?;
        let mut header = [0_u8; 2];
        stream.read_exact(&mut header).await.map_err(crate::Error::transport)?;
        let station_address = header[0];
        let parsed_function_code = header[1];
        if station_address != transaction.station_address {
            return Err(crate::Error::protocol(anyhow!("Invalid answer received.")));
        }
        if parsed_function_code == (0x80 | self.function_code.function_code()) {
            let exception_code = stream.read_u8().await?;
            return Err(crate::Error::protocol(anyhow!(ModBusException::from_code(exception_code))));
        } else if parsed_function_code != self.function_code.function_code() {
            return Err(crate::Error::protocol(anyhow!("Invalid frame")));
        }
        let fun_header_len = self.function_code.get_header_length();
        let mut fun_header = vec![0_u8; fun_header_len];
        stream.read_exact(&mut fun_header).await.map_err(crate::Error::transport)?;
        let data_len = self.function_code.get_data_length_from_header(&fun_header)?;
        let mut data = vec![0_u8; data_len + 2 + 2 + fun_header_len];
        data[0..2].copy_from_slice(&header);
        data[2..2 + fun_header_len].copy_from_slice(&fun_header);
        stream
            .read_exact(&mut data[2 + fun_header_len..])
            .await
            .map_err(crate::Error::transport)?;
        if crc(&data) != 0 {
            return Err(crate::Error::protocol(anyhow!("Invalid CRC in answer")));
        }
        self.function_code.parse_frame(&data[2 + fun_header_len..data.len() - 2])
    }
}

pub fn crc(data: &[u8]) -> u16 {
    let mut crc = 0xFFFF_u16;
    for x in data {
        crc ^= *x as u16;
        for _ in 0..8 {
            if crc & 1 != 0 {
                crc = (crc >> 1) ^ 0xA001;
            } else {
                crc >>= 1;
            }
        }
    }
    crc
}
