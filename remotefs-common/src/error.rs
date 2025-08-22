use thiserror::Error;
use crate::protocol::ErrorCode;

/// Main error type for RemoteFS operations
#[derive(Error, Debug)]
pub enum RemoteFsError {
    #[error("IO error: {0}")]
    Io(std::io::Error),
    
    #[error("Serialization error: {0}")]
    Serialization(#[from] bincode::Error),
    
    #[error("Network error: {0}")]
    Network(String),
    
    #[error("Authentication failed: {0}")]
    Authentication(String),
    
    #[error("Authorization denied: {0}")]
    Authorization(String),
    
    #[error("Access denied: {0}")]
    AccessDenied(String),
    
    #[error("File system error: {0}")]
    FileSystem(String),
    
    #[error("Encryption error: {0}")]
    Encryption(#[from] anyhow::Error),
    
    #[error("Protocol error: {0}")]
    Protocol(String),
    
    #[error("Configuration error: {0}")]
    Configuration(String),
    
    #[error("Connection error: {0}")]
    Connection(String),
    
    #[error("Session error: {0}")]
    Session(String),
    
    #[error("Timeout: {0}")]
    Timeout(String),
    
    #[error("Not found: {0}")]
    NotFound(String),
    
    #[error("Already exists: {0}")]
    AlreadyExists(String),
    
    #[error("Permission denied: {0}")]
    PermissionDenied(String),
    
    #[error("Invalid path: {0}")]
    InvalidPath(String),
    
    #[error("Service unavailable: {0}")]
    ServiceUnavailable(String),
    
    #[error("Not implemented: {0}")]
    NotImplemented(String),
    
    #[error("Internal error: {0}")]
    Internal(String),
}

impl RemoteFsError {
    /// Convert to protocol error code
    pub fn to_error_code(&self) -> ErrorCode {
        match self {
            RemoteFsError::Authentication(_) => ErrorCode::AuthenticationFailed,
            RemoteFsError::Authorization(_) => ErrorCode::AccessDenied,
            RemoteFsError::AccessDenied(_) => ErrorCode::AccessDenied,
            RemoteFsError::PermissionDenied(_) => ErrorCode::InsufficientPermissions,
            RemoteFsError::NotFound(_) => ErrorCode::FileNotFound,
            RemoteFsError::AlreadyExists(_) => ErrorCode::PathAlreadyExists,
            RemoteFsError::InvalidPath(_) => ErrorCode::InvalidPath,
            RemoteFsError::Network(_) => ErrorCode::NetworkError,
            RemoteFsError::Connection(_) => ErrorCode::NetworkError,
            RemoteFsError::Timeout(_) => ErrorCode::ConnectionTimeout,
            RemoteFsError::Protocol(_) => ErrorCode::InvalidMessage,
            RemoteFsError::ServiceUnavailable(_) => ErrorCode::ServiceUnavailable,
            RemoteFsError::NotImplemented(_) => ErrorCode::NotImplemented,
            RemoteFsError::Session(_) => ErrorCode::SessionExpired,
            _ => ErrorCode::InternalError,
        }
    }
    
    /// Create from protocol error code
    pub fn from_error_code(code: ErrorCode, message: String) -> Self {
        match code {
            ErrorCode::AuthenticationFailed => RemoteFsError::Authentication(message),
            ErrorCode::InvalidCredentials => RemoteFsError::Authentication(message),
            ErrorCode::SessionExpired => RemoteFsError::Session(message),
            ErrorCode::AccessDenied => RemoteFsError::Authorization(message),
            ErrorCode::PathNotAllowed => RemoteFsError::Authorization(message),
            ErrorCode::InsufficientPermissions => RemoteFsError::PermissionDenied(message),
            ErrorCode::FileNotFound => RemoteFsError::NotFound(message),
            ErrorCode::DirectoryNotFound => RemoteFsError::NotFound(message),
            ErrorCode::PathAlreadyExists => RemoteFsError::AlreadyExists(message),
            ErrorCode::InvalidPath => RemoteFsError::InvalidPath(message),
            ErrorCode::DiskFull => RemoteFsError::FileSystem(format!("Disk full: {}", message)),
            ErrorCode::ReadOnlyFileSystem => RemoteFsError::FileSystem(format!("Read-only filesystem: {}", message)),
            ErrorCode::NetworkError => RemoteFsError::Network(message),
            ErrorCode::ConnectionTimeout => RemoteFsError::Timeout(message),
            ErrorCode::MessageTooLarge => RemoteFsError::Protocol(format!("Message too large: {}", message)),
            ErrorCode::InvalidMessage => RemoteFsError::Protocol(message),
            ErrorCode::NotImplemented => RemoteFsError::NotImplemented(message),
            ErrorCode::ServiceUnavailable => RemoteFsError::ServiceUnavailable(message),
            ErrorCode::InternalError => RemoteFsError::Internal(message),
        }
    }
    
    /// Check if error is retryable
    pub fn is_retryable(&self) -> bool {
        matches!(self,
            RemoteFsError::Network(_) |
            RemoteFsError::Connection(_) |
            RemoteFsError::Timeout(_) |
            RemoteFsError::ServiceUnavailable(_)
        )
    }
    
    /// Check if error is temporary
    pub fn is_temporary(&self) -> bool {
        matches!(self,
            RemoteFsError::Network(_) |
            RemoteFsError::Connection(_) |
            RemoteFsError::Timeout(_) |
            RemoteFsError::ServiceUnavailable(_) |
            RemoteFsError::Session(_)
        )
    }
}

/// Result type alias for RemoteFS operations
pub type Result<T> = std::result::Result<T, RemoteFsError>;

/// Convert std::io::Error to appropriate RemoteFsError
impl From<std::io::Error> for RemoteFsError {
    fn from(error: std::io::Error) -> Self {
        match error.kind() {
            std::io::ErrorKind::NotFound => RemoteFsError::NotFound(error.to_string()),
            std::io::ErrorKind::PermissionDenied => RemoteFsError::PermissionDenied(error.to_string()),
            std::io::ErrorKind::AlreadyExists => RemoteFsError::AlreadyExists(error.to_string()),
            std::io::ErrorKind::InvalidInput => RemoteFsError::InvalidPath(error.to_string()),
            std::io::ErrorKind::TimedOut => RemoteFsError::Timeout(error.to_string()),
            std::io::ErrorKind::ConnectionRefused => RemoteFsError::Connection(error.to_string()),
            std::io::ErrorKind::ConnectionAborted => RemoteFsError::Connection(error.to_string()),
            std::io::ErrorKind::ConnectionReset => RemoteFsError::Connection(error.to_string()),
            _ => RemoteFsError::Io(error),
        }
    }
}

