//! This module defines the [`enum@Error`] enum, which separates all errors that
//! may occur in the `comsrv` into different categories.

use serde::{self, Deserializer, Serializer};
use std::{io, sync::Arc};

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::Response;

/// Encodes errors occurring in the `comsrv`. It allows separating
/// errors into 4 different categories:
///
///  * [`Error::Transport`] - Errors that occurs on the transport layer, such as a broken TCP connection or a disconnected serial port.
///  * [`Error::Protocol`] - Errors that occurs on the protocol layer, such as invalid answers or instruments failing to respond in time.
///  * [`Error::Argument`] - Is issued in case the `comsrv` receives an invalid combination of arguments.
///  * [`Error::Internal`] - If an error within the `comsrv` is encountered. Often those point to an installation error, but may also be bugs in the `comsrv`.
///
/// To avoid a very large enum that captures all errors coming from internal libraries, this enum just caputres the most cases but falls back to [`anyhow::Error`] for
/// more not-so-common errors.
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
        Arc<anyhow::Error>,
    ),
    #[error("Internal Error {0}")]
    Internal(
        #[serde(
            serialize_with = "serialize_anyhow_error",
            deserialize_with = "deserialize_anyhow_error"
        )]
        Arc<anyhow::Error>,
    ),
}

impl Error {
    pub fn transport<T: Into<TransportError>>(err: T) -> Self {
        Self::Transport(err.into())
    }

    pub fn protocol<T: Into<ProtocolError>>(err: T) -> Self {
        Self::Protocol(err.into())
    }

    /// Helper function to create `Error::Protocol(ProtocolError::Timeout)`
    pub fn protocol_timeout() -> Self {
        Error::Protocol(ProtocolError::Timeout)
    }

    /// Deteremines based on the error type, whether it makes sense (and is allowed) to retry an operation.
    pub fn should_retry(&self) -> bool {
        match self {
            Error::Transport(TransportError::Io(err)) => {
                err.kind() == io::ErrorKind::ConnectionReset
                    || err.kind() == io::ErrorKind::ConnectionAborted
                    || err.kind() == io::ErrorKind::BrokenPipe
                    || err.kind() == io::ErrorKind::TimedOut
                    || err.kind() == io::ErrorKind::UnexpectedEof
            }
            Error::Transport(TransportError::Other(_)) => false,
            Error::Protocol(_) => false,
            Error::Argument(_) => false,
            Error::Internal(_) => false,
        }
    }

    pub fn internal<T: Into<anyhow::Error>>(err: T) -> Self {
        Self::Internal(Arc::new(err.into()))
    }

    pub fn argument<T: Into<anyhow::Error>>(err: T) -> Self {
        Self::Argument(Arc::new(err.into()))
    }

    pub fn is_transport_error(&self) -> bool {
        matches!(self, Error::Transport(_))
    }
}

/// Internal struct for best-effort serialization of `anyhow::Error`.
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
    Ok(Arc::new(io::Error::new(
        io::ErrorKind::Other,
        ret.description,
    )))
}

fn serialize_anyhow_error<S>(error: &Arc<anyhow::Error>, serializer: S) -> Result<S::Ok, S::Error>
where
    S: Serializer,
{
    let to_serialize = AnyHowError {
        description: error.to_string(),
        backtrace: format!("{}", error.backtrace()),
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
    #[error("Other Error: {0}")]
    Other(
        #[serde(
            serialize_with = "serialize_anyhow_error",
            deserialize_with = "deserialize_anyhow_error"
        )]
        Arc<anyhow::Error>,
    ),
}

impl From<io::Error> for TransportError {
    fn from(err: io::Error) -> Self {
        TransportError::Io(Arc::new(err))
    }
}

impl From<anyhow::Error> for TransportError {
    fn from(x: anyhow::Error) -> Self {
        TransportError::Other(Arc::new(x))
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
    #[error("Other Error: {0}")]
    Other(
        #[serde(
            serialize_with = "serialize_anyhow_error",
            deserialize_with = "deserialize_anyhow_error"
        )]
        Arc<anyhow::Error>,
    ),
}

impl From<io::Error> for ProtocolError {
    fn from(err: io::Error) -> Self {
        ProtocolError::Io(Arc::new(err))
    }
}

impl From<anyhow::Error> for ProtocolError {
    fn from(x: anyhow::Error) -> Self {
        ProtocolError::Other(Arc::new(x))
    }
}

impl From<Error> for Response {
    fn from(val: Error) -> Self {
        Response::Error(val)
    }
}

impl From<io::Error> for Error {
    fn from(x: io::Error) -> Self {
        Error::Transport(TransportError::Io(Arc::new(x)))
    }
}
