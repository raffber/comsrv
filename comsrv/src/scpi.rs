/// This module implements some base types and functions to interact with SCPI-based instruments
use anyhow::anyhow;

fn invalid_binary_header() -> crate::Error {
    crate::Error::protocol(anyhow!("Invalid binary header"))
}

/// Parse an SCPI binary header.
pub fn parse_binary_header(rx: &[u8]) -> crate::Result<(usize, usize)> {
    let begin = rx
        .iter()
        .position(|x| *x == b'#')
        .ok_or(invalid_binary_header())?;

    const DEFAULT_LENGTH_BEFORE_BLOCK: usize = 25;

    if begin > DEFAULT_LENGTH_BEFORE_BLOCK {
        return Err(invalid_binary_header());
    }
    let header_length = if rx.len() < begin + 2 {
        0
    } else {
        let data = String::from_utf8(vec![rx[begin + 1]]).map_err(|_| invalid_binary_header())?;
        data.parse::<usize>().map_err(|_| invalid_binary_header())?
    };
    let offset = begin + 2 + header_length;
    if offset > rx.len() {
        return Err(invalid_binary_header());
    }
    let data_length = if header_length > 0 {
        let x: Vec<_> = rx[begin + 2..offset].to_vec();
        let data = String::from_utf8(x).map_err(|_| invalid_binary_header())?;
        data.parse::<usize>().map_err(|_| invalid_binary_header())?
    } else {
        0
    };
    if offset + data_length > rx.len() {
        Err(invalid_binary_header())
    } else {
        Ok((offset, data_length))
    }
}
