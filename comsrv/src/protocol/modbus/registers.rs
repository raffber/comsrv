use super::FunctionCode;
use anyhow::anyhow;

pub struct ReadU16Registers {
    function_code: u8,
    address: u16,
    cnt: u8,
}

impl ReadU16Registers {
    pub fn new(function_code: u8, address: u16, cnt: u8) -> crate::Result<Self> {
        if cnt == 0 {
            return Err(crate::Error::argument(anyhow!("Need to read at least 1 register.")));
        }
        if cnt > 125 {
            return Err(crate::Error::argument(anyhow!(
                "Trying to read too many registers: {}. Maximum 125.",
                cnt
            )));
        }
        Ok(Self {
            function_code,
            address,
            cnt,
        })
    }
}

impl FunctionCode for ReadU16Registers {
    type Output = Vec<u16>;

    fn format_request(&self, data: &mut Vec<u8>) {
        data.extend(&self.address.to_be_bytes());
        data.extend(&(self.cnt as u16).to_be_bytes());
    }

    fn get_header_length(&self) -> usize {
        1
    }

    fn get_data_length_from_header(&self, data: &[u8]) -> crate::Result<usize> {
        let len = data[0] as usize;
        if len < 2 * self.cnt as usize {
            return Err(crate::Error::protocol(anyhow!("Invalid receive frame length")));
        }
        Ok(len)
    }

    fn parse_frame(&self, data: &[u8]) -> crate::Result<Self::Output> {
        let truncated = &data[0..2 * self.cnt as usize];
        let mut ret = Vec::with_capacity(self.cnt as usize);
        for x in truncated.chunks(2).take(self.cnt as usize) {
            let value = u16::from_be_bytes([x[0], x[1]]);
            ret.push(value);
        }
        Ok(ret)
    }

    fn function_code(&self) -> u8 {
        self.function_code
    }
}

pub struct ReadBoolRegisters {
    function_code: u8,
    address: u16,
    cnt: u16,
}

impl ReadBoolRegisters {
    pub fn new(function_code: u8, address: u16, cnt: u16) -> crate::Result<Self> {
        if cnt == 0 {
            return Err(crate::Error::argument(anyhow!("Need to read at least 1 register")));
        }
        if cnt > 1968 {
            return Err(crate::Error::argument(anyhow!(
                "Trying to read too many registers: {}. Maximum 1968",
                cnt
            )));
        }
        Ok(Self {
            function_code,
            address,
            cnt,
        })
    }
}

impl FunctionCode for ReadBoolRegisters {
    type Output = Vec<bool>;

    fn format_request(&self, data: &mut Vec<u8>) {
        data.extend(&self.address.to_be_bytes());
        data.extend(&self.cnt.to_be_bytes());
    }

    fn get_header_length(&self) -> usize {
        1
    }

    fn get_data_length_from_header(&self, data: &[u8]) -> crate::Result<usize> {
        let expected_byte_count = ((self.cnt - 1) / 8) + 1;
        let len = data[0] as usize;
        if len < expected_byte_count as usize {
            return Err(crate::Error::protocol(anyhow!("Invalid receive frame length")));
        }
        Ok(len)
    }

    fn parse_frame(&self, data: &[u8]) -> crate::Result<Self::Output> {
        let expected_byte_count = ((self.cnt - 1) / 8) + 1;
        let truncated = &data[0..expected_byte_count as usize];
        let mut ret = Vec::new();
        'outer: for x in truncated {
            let mut x = *x;
            for _ in 0..8 {
                ret.push((x & 1) == 1);
                if ret.len() == expected_byte_count as usize {
                    break 'outer;
                }
                x = x >> 1;
            }
        }
        Ok(ret)
    }

    fn function_code(&self) -> u8 {
        self.function_code
    }
}

pub struct WriteCoils<'a> {
    address: u16,
    data: &'a [bool],
}

impl<'a> WriteCoils<'a> {
    pub fn new(address: u16, data: &'a [bool]) -> crate::Result<Self> {
        if data.len() == 0 {
            return Err(crate::Error::argument(anyhow!("Number of write coils must be > 0")));
        }
        if data.len() > 0x7B0 {
            return Err(crate::Error::argument(anyhow!("Number of write coils must be <= 1968")));
        }
        Ok(Self { address, data })
    }
}

impl<'a> FunctionCode for WriteCoils<'a> {
    type Output = ();

    fn format_request(&self, data: &mut Vec<u8>) {
        data.extend(self.address.to_be_bytes());
        data.extend((self.data.len() as u16).to_be_bytes());
        for chunk in self.data.chunks(8) {
            let mut byte: u8 = 0;
            let mut k = 0;
            for x in chunk {
                if *x {
                    byte |= 1 << k;
                }
                k += 1;
            }
            data.push(byte);
        }
    }

    fn get_header_length(&self) -> usize {
        4
    }

    fn get_data_length_from_header(&self, data: &[u8]) -> crate::Result<usize> {
        check_write_header(data, self.address, self.data.len())?;
        Ok(0)
    }

    fn parse_frame(&self, _data: &[u8]) -> crate::Result<Self::Output> {
        Ok(())
    }

    fn function_code(&self) -> u8 {
        15
    }
}

pub struct WriteRegisters<'a> {
    address: u16,
    data: &'a [u16],
}

impl<'a> WriteRegisters<'a> {
    pub fn new(address: u16, data: &'a [u16]) -> crate::Result<Self> {
        if data.len() == 0 {
            return Err(crate::Error::argument(anyhow!("Number of write coils must be > 0")));
        }
        if data.len() > 125 {
            return Err(crate::Error::argument(anyhow!("Number of write coils must be <= 125")));
        }
        Ok(Self { address, data })
    }
}

impl<'a> FunctionCode for WriteRegisters<'a> {
    type Output = ();

    fn format_request(&self, data: &mut Vec<u8>) {
        data.extend(self.address.to_be_bytes());
        data.extend((self.data.len() as u16).to_be_bytes());
        data.push(2 * self.data.len() as u8);
        for x in self.data {
            data.extend(&x.to_be_bytes());
        }
    }

    fn get_header_length(&self) -> usize {
        4
    }

    fn get_data_length_from_header(&self, data: &[u8]) -> crate::Result<usize> {
        check_write_header(data, self.address, self.data.len())?;
        Ok(0)
    }

    fn parse_frame(&self, _data: &[u8]) -> crate::Result<Self::Output> {
        Ok(())
    }

    fn function_code(&self) -> u8 {
        16
    }
}

fn check_write_header(reply: &[u8], addr: u16, num_regs: usize) -> crate::Result<()> {
    let starting_address = u16::from_be_bytes([reply[0], reply[1]]);
    let num_outputs = u16::from_be_bytes([reply[2], reply[3]]);
    if starting_address != addr {
        return Err(crate::Error::protocol(anyhow!("Unexpected Answer")));
    }
    if num_regs != num_outputs as usize {
        return Err(crate::Error::protocol(anyhow!("Unexpected register length")));
    }
    return Ok(());
}
