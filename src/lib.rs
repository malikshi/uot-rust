// src/lib.rs

pub mod client;
pub mod error;
pub mod protocol;
pub mod server;

// Re-export key types for easy access
pub use client::UotConn;
pub use error::UotError;
pub use protocol::{SocksAddr, UotRequest};
