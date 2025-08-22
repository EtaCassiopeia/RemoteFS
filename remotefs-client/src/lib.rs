//! RemoteFS Client Library
//!
//! A Rust client library for connecting to and interacting with RemoteFS agents.
//! Provides high-level filesystem operations over WebSocket connections with
//! support for load balancing, retries, and connection pooling.

mod client;
mod config;
mod connection;
mod error;

pub use client::*;
pub use config::*;
pub use connection::*;
pub use error::*;

// Type alias for convenience
pub type Client = RemoteFsClient;

// Re-export common types for convenience
pub use remotefs_common::{error::RemoteFsError, protocol::*};
