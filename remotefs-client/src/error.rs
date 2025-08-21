use thiserror::Error;
use remotefs_common::error::RemoteFsError;

/// Client-specific error types
#[derive(Error, Debug)]
pub enum ClientError {
    #[error("Connection error: {0}")]
    Connection(String),
    
    #[error("Authentication failed: {0}")]
    Authentication(String),
    
    #[error("Network error: {0}")]
    Network(#[from] tokio_tungstenite::tungstenite::Error),
    
    #[error("Serialization error: {0}")]
    Serialization(#[from] bincode::Error),
    
    #[error("Configuration error: {0}")]
    Configuration(String),
    
    #[error("Timeout: operation timed out after {seconds} seconds")]
    Timeout { seconds: u64 },
    
    #[error("Agent unavailable: {message}")]
    AgentUnavailable { message: String },
    
    #[error("Invalid response: {0}")]
    InvalidResponse(String),
    
    #[error("Remote filesystem error: {0}")]
    RemoteFs(#[from] RemoteFsError),
    
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    
    #[error("URL parse error: {0}")]
    UrlParse(#[from] url::ParseError),
    
    #[error("Internal error: {0}")]
    Internal(String),
}

impl ClientError {
    /// Check if the error is retryable
    pub fn is_retryable(&self) -> bool {
        match self {
            ClientError::Network(_) => true,
            ClientError::Connection(_) => true,
            ClientError::Timeout { .. } => true,
            ClientError::AgentUnavailable { .. } => true,
            ClientError::RemoteFs(e) => e.is_retryable(),
            _ => false,
        }
    }
    
    /// Check if the error is temporary
    pub fn is_temporary(&self) -> bool {
        match self {
            ClientError::Network(_) => true,
            ClientError::Connection(_) => true,
            ClientError::Timeout { .. } => true,
            ClientError::AgentUnavailable { .. } => true,
            ClientError::RemoteFs(e) => e.is_temporary(),
            _ => false,
        }
    }
}

/// Result type for client operations
pub type ClientResult<T> = Result<T, ClientError>;
