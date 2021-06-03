use core::fmt;
use std::fmt::{Display, Formatter};
use std::time::Duration;

use serde::{Deserialize, Serialize};
use tokio_serial::{FlowControl, SerialPortSettings};

use crate::serial::DEFAULT_TIMEOUT_MS;

#[derive(PartialEq, Clone, Copy, Serialize, Deserialize, Hash)]
pub enum StopBits {
    One,
    Two,
}

#[derive(PartialEq, Clone, Copy, Serialize, Deserialize, Hash)]
pub enum Parity {
    None,
    Odd,
    Even,
}

#[derive(PartialEq, Clone, Copy, Serialize, Deserialize, Hash)]
pub enum DataBits {
    Five,
    Six,
    Seven,
    Eight,
}

pub fn parse_serial_settings(settings: &str) -> Option<(DataBits, Parity, StopBits)> {
    let settings = settings.to_lowercase();
    let chars = settings.as_bytes();
    if chars.len() != 3 {
        return None;
    }
    let data_bits = match chars[0] as char {
        '8' => DataBits::Eight,
        '7' => DataBits::Seven,
        '6' => DataBits::Six,
        '5' => DataBits::Five,
        _ => return None,
    };
    let parity = match chars[1] as char {
        'n' => Parity::None,
        'o' => Parity::Odd,
        'e' => Parity::Even,
        _ => return None,
    };
    let stop_bits = match chars[2] as char {
        '1' => StopBits::One,
        '2' => StopBits::Two,
        _ => return None,
    };
    Some((data_bits, parity, stop_bits))
}

#[derive(PartialEq, Clone, Serialize, Deserialize, Hash)]
pub struct SerialParams {
    pub baud: u32,
    pub data_bits: DataBits,
    pub stop_bits: StopBits,
    pub parity: Parity,
}

impl SerialParams {
    pub fn from_address(addr_parts: &[&str]) -> Option<(String, SerialParams)> {
        if addr_parts.len() != 3 {
            return None;
        }
        let path = addr_parts[0].into();
        let baud_rate: u32 = addr_parts[1].parse().ok()?;
        let (bits, parity, stop) = parse_serial_settings(&splits[2])?;
        Some((
            path,
            SerialParams {
                baud: baud_rate,
                data_bits: bits,
                stop_bits: stop,
                parity,
            },
        ))
    }
}

impl ToString for SerialParams {
    fn to_string(&self) -> String {
        format!("{}::{}{}{}", baud, data_bits, parity, stop_bits)
    }
}

impl Display for SerialParams {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.write_str(&self.to_string())
    }
}


impl Into<SerialPortSettings> for SerialParams {
    fn into(self) -> SerialPortSettings {
        SerialPortSettings {
            baud_rate: self.baud,
            data_bits: tokio_serial::DataBits::Eight,
            flow_control: FlowControl::None,
            parity: self.parity.into(),
            stop_bits: self.stop_bits.into(),
            timeout: Duration::from_millis(DEFAULT_TIMEOUT_MS),
        }
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

impl Into<tokio_serial::StopBits> for StopBits {
    fn into(self) -> tokio_serial::StopBits {
        match self {
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

impl Into<tokio_serial::Parity> for Parity {
    fn into(self) -> tokio_serial::Parity {
        match self {
            Parity::None => tokio_serial::Parity::None,
            Parity::Odd => tokio_serial::Parity::Odd,
            Parity::Even => tokio_serial::Parity::Even,
        }
    }
}

impl From<tokio_serial::DataBits> for DataBits {
    fn from(x: tokio_serial::DataBits) -> Self {
        match x {
            tokio_serial::DataBits::Five => DataBits::Five,
            tokio_serial::DataBits::Six => DataBits::Six,
            tokio_serial::DataBits::Seven => DataBits::Seven,
            tokio_serial::DataBits::Eight => DataBits::Eight,
        }
    }
}

impl Into<tokio_serial::DataBits> for DataBits {
    fn into(self) -> tokio_serial::DataBits {
        match self {
            DataBits::Five => tokio_serial::DataBits::Five,
            DataBits::Six => tokio_serial::DataBits::Six,
            DataBits::Seven => tokio_serial::DataBits::Seven,
            DataBits::Eight => tokio_serial::DataBits::Eight,
        }
    }
}

impl Display for DataBits {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        let x = match self {
            DataBits::Five => "5",
            DataBits::Six => "6",
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
