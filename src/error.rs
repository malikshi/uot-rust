// src/error.rs
use thiserror::Error;

#[derive(Error, Debug)]
pub enum UotError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Protocol error: {0}")]
    Protocol(String),

    #[error("Address resolution failed")]
    ResolutionFailed,

    #[error("Unknown protocol version: {0}")]
    UnknownVersion(u8),
}
