use core::fmt;
use std::{
    convert::TryInto,
    fmt::{Display, Formatter},
};

use crate::rpc::FlowControl;
use anyhow::anyhow;
use comsrv_protocol::SerialPortConfig;
use serde::{Deserialize, Serialize};

#[derive(Debug, Eq, PartialEq, Clone, Copy, Serialize, Deserialize, Hash)]
pub enum StopBits {
    One,
    Two,
}

#[derive(Debug, Eq, PartialEq, Clone, Copy, Serialize, Deserialize, Hash)]
pub enum Parity {
    None,
    Odd,
    Even,
}

#[derive(Debug, Eq, PartialEq, Clone, Copy, Serialize, Deserialize, Hash)]
pub enum DataBits {
    Seven,
    Eight,
}

pub fn parse_serial_settings(settings: &str) -> crate::Result<(DataBits, Parity, StopBits)> {
    let settings = settings.to_lowercase();
    let chars = settings.as_bytes();
    if chars.len() != 3 {
        return Err(crate::Error::argument(anyhow!("Invalid Address")));
    }
    let data_bits = match chars[0] as char {
        '8' => DataBits::Eight,
        '7' => DataBits::Seven,
        _ => return Err(crate::Error::argument(anyhow!("Invalid Address"))),
    };
    let parity = match chars[1] as char {
        'n' => Parity::None,
        'o' => Parity::Odd,
        'e' => Parity::Even,
        _ => return Err(crate::Error::argument(anyhow!("Invalid Address"))),
    };
    let stop_bits = match chars[2] as char {
        '1' => StopBits::One,
        '2' => StopBits::Two,
        _ => return Err(crate::Error::argument(anyhow!("Invalid Address"))),
    };
    Ok((data_bits, parity, stop_bits))
}

#[derive(Debug, Eq, PartialEq, Clone, Serialize, Deserialize, Hash)]
pub struct SerialParams {
    pub baud: u32,
    pub data_bits: DataBits,
    pub stop_bits: StopBits,
    pub parity: Parity,
    pub hardware_flow_control: FlowControl,
}

impl TryInto<SerialParams> for SerialPortConfig {
    type Error = crate::Error;

    fn try_into(self) -> Result<SerialParams, Self::Error> {
        let (data_bits, parity, stop_bits) = parse_serial_settings(&self.config)?;
        Ok(SerialParams {
            baud: self.baudrate,
            data_bits,
            stop_bits,
            parity,
            hardware_flow_control: self.hardware_flow_control,
        })
    }
}

impl Display for SerialParams {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        let x = format!("{}::{}{}{}", self.baud, self.data_bits, self.parity, self.stop_bits);
        f.write_str(&x)
    }
}

impl From<tokio_serial::StopBits> for StopBits {
    fn from(x: tokio_serial::StopBits) -> Self {
        match x {
            tokio_serial::StopBits::One => StopBits::One,
            tokio_serial::StopBits::Two => StopBits::Two,
        }
    }
}

impl From<StopBits> for tokio_serial::StopBits {
    fn from(x: StopBits) -> Self {
        match x {
            StopBits::One => tokio_serial::StopBits::One,
            StopBits::Two => tokio_serial::StopBits::Two,
        }
    }
}

impl From<tokio_serial::Parity> for Parity {
    fn from(x: tokio_serial::Parity) -> Self {
        match x {
            tokio_serial::Parity::None => Parity::None,
            tokio_serial::Parity::Odd => Parity::Odd,
            tokio_serial::Parity::Even => Parity::Even,
        }
    }
}

#[allow(clippy::from_over_into)]
impl Into<tokio_serial::Parity> for Parity {
    fn into(self) -> tokio_serial::Parity {
        match self {
            Parity::None => tokio_serial::Parity::None,
            Parity::Odd => tokio_serial::Parity::Odd,
            Parity::Even => tokio_serial::Parity::Even,
        }
    }
}

#[allow(clippy::from_over_into)]
impl Into<tokio_serial::DataBits> for DataBits {
    fn into(self) -> tokio_serial::DataBits {
        match self {
            DataBits::Seven => tokio_serial::DataBits::Seven,
            DataBits::Eight => tokio_serial::DataBits::Eight,
        }
    }
}

impl Display for DataBits {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        let x = match self {
            DataBits::Seven => "7",
            DataBits::Eight => "8",
        };
        f.write_str(x)
    }
}

impl Display for Parity {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        let x = match self {
            Parity::None => "N",
            Parity::Odd => "O",
            Parity::Even => "E",
        };
        f.write_str(x)
    }
}

impl Display for StopBits {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        let x = match self {
            StopBits::One => "1",
            StopBits::Two => "2",
        };
        f.write_str(x)
    }
}

impl From<SerialParams> for SerialPortConfig {
    fn from(params: SerialParams) -> Self {
        SerialPortConfig {
            config: format!("{}{}{}", params.data_bits, params.parity, params.stop_bits),
            baudrate: params.baud,
            hardware_flow_control: params.hardware_flow_control,
        }
    }
}
