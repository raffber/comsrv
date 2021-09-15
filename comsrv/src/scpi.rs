/// This module implements some base types and functions to interact with SCPI-based instruments
use crate::Error;

/// Parse an SCPI binary header.
pub fn parse_binary_header(rx: &[u8]) -> crate::Result<(usize, usize)> {
    let begin = rx
        .iter()
        .position(|x| *x == b'#')
        .ok_or(Error::InvalidBinaryHeader)?;

    const DEFAULT_LENGTH_BEFORE_BLOCK: usize = 25;

    if begin > DEFAULT_LENGTH_BEFORE_BLOCK {
        return Err(Error::InvalidBinaryHeader);
    }
    let header_length = if rx.len() < begin + 2 {
        0
    } else {
        let data =
            String::from_utf8(vec![rx[begin + 1]]).map_err(|_| Error::InvalidBinaryHeader)?;
        data.parse::<usize>()
            .map_err(|_| Error::InvalidBinaryHeader)?
    };
    let offset = begin + 2 + header_length;
    if offset > rx.len() {
        return Err(Error::InvalidBinaryHeader);
    }
    let data_length = if header_length > 0 {
        let x: Vec<_> = rx[begin + 2..offset].to_vec();
        let data = String::from_utf8(x).map_err(|_| Error::InvalidBinaryHeader)?;
        data.parse::<usize>()
            .map_err(|_| Error::InvalidBinaryHeader)?
    } else {
        0
    };
    if offset + data_length > rx.len() {
        Err(Error::InvalidBinaryHeader)
    } else {
        Ok((offset, data_length))
    }
}
