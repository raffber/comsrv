use serde::{self, Deserializer, Serializer};
use std::{io, sync::Arc};

use serde::{Deserialize, Serialize};
use thiserror::Error;

fn serialize_io_error<S>(error: &Arc<io::Error>, serializer: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    todo!()
}

fn deserialize_io_error<'de, D>(deserializer: D) -> Result<Arc<io::Error>, D::Error>
where
    D: Deserializer<'de>,
{
    todo!()
}

fn serialize_anyhow_error<S>(error: &Arc<anyhow::Error>, serializer: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    todo!()
}

fn deserialize_anyhow_error<'de, D>(deserializer: D) -> Result<Arc<anyhow::Error>, D::Error>
where
    D: Deserializer<'de>,
{
    todo!()
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


