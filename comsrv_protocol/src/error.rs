use serde::{self, Deserializer, Serializer};
use std::{io, sync::Arc};

use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Serialize, Deserialize)]
struct AnyHowError {
    description: String,
    backtrace: String,
}

#[derive(Serialize, Deserialize)]
struct IoError {
    description: String,
    kind: String,
}

fn serialize_io_error<S>(error: &Arc<io::Error>, serializer: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    let to_serialize = IoError {
        description: format!("{}", error),
        kind: format!("{}", error.kind()),
    };
    to_serialize.serialize(serializer)
}

fn deserialize_io_error<'de, D>(deserializer: D) -> Result<Arc<io::Error>, D::Error>
where
    D: Deserializer<'de>,
{
    let ret = IoError::deserialize(deserializer)?;
    // TODO: deserialize ErrorKind
    Ok(Arc::new(io::Error::new(io::ErrorKind::Other, ret.description)))
}

fn serialize_anyhow_error<S>(error: &Arc<anyhow::Error>, serializer: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    let to_serialize = AnyHowError {
        description: error.to_string(),
        backtrace: format!("{:?}", error.backtrace()),
    };
    to_serialize.serialize(serializer)
}

fn deserialize_anyhow_error<'de, D>(deserializer: D) -> Result<Arc<anyhow::Error>, D::Error>
where
    D: Deserializer<'de>,
{
    let ret = AnyHowError::deserialize(deserializer)?;
    let ret = anyhow::Error::msg(ret.description);
    Ok(Arc::new(ret))
}

#[derive(Error, Clone, Debug, Serialize, Deserialize)]
pub enum TransportError {
    #[error("IO Error: {0:?}")]
    Io(
        #[serde(
            serialize_with = "serialize_io_error",
            deserialize_with = "deserialize_io_error"
        )]
        Arc<io::Error>,
    ),
}

impl From<io::Error> for TransportError {
    fn from(err: io::Error) -> Self {
        TransportError::Io(Arc::new(err))
    }
}

#[derive(Error, Clone, Debug, Serialize, Deserialize)]
pub enum ProtocolError {
    #[error("IO Error: {0:?}")]
    Io(
        #[serde(
            serialize_with = "serialize_io_error",
            deserialize_with = "deserialize_io_error"
        )]
        Arc<io::Error>,
    ),
    #[error("Timeout")]
    Timeout,
    #[error("Unexpected Response: {0}")]
    UnexpectedResponse(String),
}

impl From<io::Error> for ProtocolError {
    fn from(err: io::Error) -> Self {
        ProtocolError::Io(Arc::new(err))
    }
}

#[derive(Error, Debug, Clone, Serialize, Deserialize)]
pub enum Error {
    #[error("Transport Error {0}")]
    Transport(TransportError),
    #[error("Protocol Error {0}")]
    Protocol(ProtocolError),
    #[error("Argument Error {0}")]
    Argument(
        #[serde(
            serialize_with = "serialize_anyhow_error",
            deserialize_with = "deserialize_anyhow_error"
        )]
        Arc<anyhow::Error>),
    #[error("Internal Error {0}")]
    Internal(
        #[serde(
            serialize_with = "serialize_anyhow_error",
            deserialize_with = "deserialize_anyhow_error"
        )]
        Arc<anyhow::Error>),
}


