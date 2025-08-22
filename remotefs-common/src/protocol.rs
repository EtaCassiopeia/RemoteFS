use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;
use bytes::Bytes;
use chrono::{DateTime, Utc};

/// Unique identifier for a request-response pair
pub type RequestId = Uuid;

/// Client/Agent identifier 
pub type NodeId = String;

/// Session token for authenticated connections
pub type SessionToken = String;

/// File system path
pub type FsPath = String;

/// File type enumeration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum FileType {
    File,
    Directory, 
    Symlink,
    BlockDevice,
    CharDevice,
    Fifo,
    Socket,
}

/// File metadata structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileMetadata {
    pub size: u64,
    pub modified: DateTime<Utc>,
    pub created: DateTime<Utc>,
    pub accessed: DateTime<Utc>,
    pub permissions: u32,
    pub uid: u32,
    pub gid: u32,
    pub is_dir: bool,
    pub is_file: bool,
    pub is_symlink: bool,
    pub file_type: FileType,
    pub symlink_target: Option<String>,
}

/// Directory entry information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DirEntry {
    pub name: String,
    pub metadata: FileMetadata,
}

/// Connection information for relay server
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RelayInfo {
    pub relay_id: String,
    pub capabilities: Vec<String>,
    pub max_message_size: u64,
    pub heartbeat_interval: u64,
}

/// Main message types for communication between all components
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Message {
    // ===== Authentication & Connection Management =====
    
    /// Initial authentication request from client/agent to relay
    AuthRequest {
        node_id: NodeId,
        node_type: NodeType,
        public_key: Vec<u8>,
        capabilities: Vec<String>,
    },
    
    /// Authentication response from relay
    AuthResponse {
        success: bool,
        session_token: Option<SessionToken>,
        relay_info: Option<RelayInfo>,
        error: Option<String>,
    },
    
    /// Request to establish secure channel between client and agent
    EstablishChannel {
        target_node: NodeId,
        encrypted_key_exchange: Vec<u8>,
    },
    
    /// Response to channel establishment
    ChannelEstablished {
        success: bool,
        encrypted_response: Option<Vec<u8>>,
        error: Option<String>,
    },
    
    // ===== File System Operations =====
    
    /// Read data from a file
    ReadFile {
        request_id: RequestId,
        path: FsPath,
        offset: u64,
        length: u32,
    },
    
    /// Response to read operation
    ReadFileResponse {
        request_id: RequestId,
        success: bool,
        data: Option<Vec<u8>>,
        bytes_read: u64,
        error: Option<String>,
    },
    
    /// Write data to a file
    WriteFile {
        request_id: RequestId,
        path: FsPath,
        offset: u64,
        data: Vec<u8>,
        sync: bool, // Whether to sync immediately
    },
    
    /// Response to write operation  
    WriteFileResponse {
        request_id: RequestId,
        success: bool,
        bytes_written: u64,
        error: Option<String>,
    },
    
    /// Create a new file
    CreateFile {
        request_id: RequestId,
        path: FsPath,
        mode: u32,
        exclusive: bool,
    },
    
    /// Response to create operation
    CreateFileResponse {
        request_id: RequestId,
        success: bool,
        metadata: Option<FileMetadata>,
        error: Option<String>,
    },
    
    /// Delete a file
    DeleteFile {
        request_id: RequestId,
        path: FsPath,
    },
    
    /// Response to delete operation
    DeleteFileResponse {
        request_id: RequestId,
        success: bool,
        error: Option<String>,
    },
    
    /// Truncate a file to specified size
    TruncateFile {
        request_id: RequestId,
        path: FsPath,
        size: u64,
    },
    
    /// Response to truncate operation
    TruncateFileResponse {
        request_id: RequestId,
        success: bool,
        error: Option<String>,
    },
    
    // ===== Directory Operations =====
    
    /// List directory contents
    ListDirectory {
        request_id: RequestId,
        path: FsPath,
    },
    
    /// Response to directory listing
    ListDirectoryResponse {
        request_id: RequestId,
        success: bool,
        entries: Option<Vec<DirEntry>>,
        error: Option<String>,
    },
    
    /// Create a directory
    CreateDirectory {
        request_id: RequestId,
        path: FsPath,
        mode: u32,
    },
    
    /// Response to create directory
    CreateDirectoryResponse {
        request_id: RequestId,
        success: bool,
        metadata: Option<FileMetadata>,
        error: Option<String>,
    },
    
    /// Remove a directory
    RemoveDirectory {
        request_id: RequestId,
        path: FsPath,
        recursive: bool,
    },
    
    /// Response to remove directory
    RemoveDirectoryResponse {
        request_id: RequestId,
        success: bool,
        error: Option<String>,
    },
    
    // ===== Metadata Operations =====
    
    /// Get file/directory metadata
    GetMetadata {
        request_id: RequestId,
        path: FsPath,
        follow_symlinks: bool,
    },
    
    /// Response to metadata request
    GetMetadataResponse {
        request_id: RequestId,
        success: bool,
        metadata: Option<FileMetadata>,
        error: Option<String>,
    },
    
    /// Set file/directory metadata
    SetMetadata {
        request_id: RequestId,
        path: FsPath,
        metadata: FileMetadata,
    },
    
    /// Response to set metadata
    SetMetadataResponse {
        request_id: RequestId,
        success: bool,
        error: Option<String>,
    },
    
    /// Rename/move a file or directory
    Rename {
        request_id: RequestId,
        from_path: FsPath,
        to_path: FsPath,
    },
    
    /// Response to rename operation
    RenameResponse {
        request_id: RequestId,
        success: bool,
        error: Option<String>,
    },
    
    /// Create a symbolic link
    CreateSymlink {
        request_id: RequestId,
        link_path: FsPath,
        target_path: FsPath,
    },
    
    /// Response to symlink creation
    CreateSymlinkResponse {
        request_id: RequestId,
        success: bool,
        error: Option<String>,
    },
    
    // ===== System Operations =====
    
    /// Check if path exists
    PathExists {
        request_id: RequestId,
        path: FsPath,
    },
    
    /// Response to path existence check
    PathExistsResponse {
        request_id: RequestId,
        exists: bool,
        error: Option<String>,
    },
    
    /// Get file system space information
    GetSpaceInfo {
        request_id: RequestId,
        path: FsPath,
    },
    
    /// Response to space info request
    GetSpaceInfoResponse {
        request_id: RequestId,
        success: bool,
        total_space: Option<u64>,
        available_space: Option<u64>,
        used_space: Option<u64>,
        error: Option<String>,
    },
    
    // ===== Connection Management =====
    
    /// Heartbeat/keepalive message
    Ping {
        timestamp: DateTime<Utc>,
    },
    
    /// Response to ping
    Pong {
        timestamp: DateTime<Utc>,
        original_timestamp: DateTime<Utc>,
    },
    
    /// Notify about connection closure
    ConnectionClose {
        reason: String,
    },
    
    /// Generic error message
    Error {
        request_id: Option<RequestId>,
        code: ErrorCode,
        message: String,
        details: Option<HashMap<String, String>>,
    },
}

/// Type of node in the network
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum NodeType {
    Client,
    Agent,
    Relay,
}

/// Standard error codes for the protocol
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ErrorCode {
    // Authentication errors
    AuthenticationFailed,
    InvalidCredentials,
    SessionExpired,
    
    // Authorization errors  
    AccessDenied,
    PathNotAllowed,
    InsufficientPermissions,
    
    // File system errors
    FileNotFound,
    DirectoryNotFound,
    PathAlreadyExists,
    InvalidPath,
    DiskFull,
    ReadOnlyFileSystem,
    
    // Network/Communication errors
    NetworkError,
    ConnectionTimeout,
    MessageTooLarge,
    InvalidMessage,
    
    // System errors
    InternalError,
    NotImplemented,
    ServiceUnavailable,
}

impl Message {
    /// Extract request ID from message if present
    pub fn request_id(&self) -> Option<RequestId> {
        match self {
            Message::ReadFile { request_id, .. } => Some(*request_id),
            Message::ReadFileResponse { request_id, .. } => Some(*request_id),
            Message::WriteFile { request_id, .. } => Some(*request_id),
            Message::WriteFileResponse { request_id, .. } => Some(*request_id),
            Message::CreateFile { request_id, .. } => Some(*request_id),
            Message::CreateFileResponse { request_id, .. } => Some(*request_id),
            Message::DeleteFile { request_id, .. } => Some(*request_id),
            Message::DeleteFileResponse { request_id, .. } => Some(*request_id),
            Message::TruncateFile { request_id, .. } => Some(*request_id),
            Message::TruncateFileResponse { request_id, .. } => Some(*request_id),
            Message::ListDirectory { request_id, .. } => Some(*request_id),
            Message::ListDirectoryResponse { request_id, .. } => Some(*request_id),
            Message::CreateDirectory { request_id, .. } => Some(*request_id),
            Message::CreateDirectoryResponse { request_id, .. } => Some(*request_id),
            Message::RemoveDirectory { request_id, .. } => Some(*request_id),
            Message::RemoveDirectoryResponse { request_id, .. } => Some(*request_id),
            Message::GetMetadata { request_id, .. } => Some(*request_id),
            Message::GetMetadataResponse { request_id, .. } => Some(*request_id),
            Message::SetMetadata { request_id, .. } => Some(*request_id),
            Message::SetMetadataResponse { request_id, .. } => Some(*request_id),
            Message::Rename { request_id, .. } => Some(*request_id),
            Message::RenameResponse { request_id, .. } => Some(*request_id),
            Message::CreateSymlink { request_id, .. } => Some(*request_id),
            Message::CreateSymlinkResponse { request_id, .. } => Some(*request_id),
            Message::PathExists { request_id, .. } => Some(*request_id),
            Message::PathExistsResponse { request_id, .. } => Some(*request_id),
            Message::GetSpaceInfo { request_id, .. } => Some(*request_id),
            Message::GetSpaceInfoResponse { request_id, .. } => Some(*request_id),
            Message::Error { request_id, .. } => *request_id,
            _ => None,
        }
    }
    
    /// Check if this is a response message
    pub fn is_response(&self) -> bool {
        matches!(self,
            Message::AuthResponse { .. } |
            Message::ChannelEstablished { .. } |
            Message::ReadFileResponse { .. } |
            Message::WriteFileResponse { .. } |
            Message::CreateFileResponse { .. } |
            Message::DeleteFileResponse { .. } |
            Message::TruncateFileResponse { .. } |
            Message::ListDirectoryResponse { .. } |
            Message::CreateDirectoryResponse { .. } |
            Message::RemoveDirectoryResponse { .. } |
            Message::GetMetadataResponse { .. } |
            Message::SetMetadataResponse { .. } |
            Message::RenameResponse { .. } |
            Message::CreateSymlinkResponse { .. } |
            Message::PathExistsResponse { .. } |
            Message::GetSpaceInfoResponse { .. } |
            Message::Pong { .. } |
            Message::Error { .. }
        )
    }
    
    /// Get message type name for logging
    pub fn message_type(&self) -> &'static str {
        match self {
            Message::AuthRequest { .. } => "AuthRequest",
            Message::AuthResponse { .. } => "AuthResponse", 
            Message::EstablishChannel { .. } => "EstablishChannel",
            Message::ChannelEstablished { .. } => "ChannelEstablished",
            Message::ReadFile { .. } => "ReadFile",
            Message::ReadFileResponse { .. } => "ReadFileResponse",
            Message::WriteFile { .. } => "WriteFile",
            Message::WriteFileResponse { .. } => "WriteFileResponse",
            Message::CreateFile { .. } => "CreateFile",
            Message::CreateFileResponse { .. } => "CreateFileResponse",
            Message::DeleteFile { .. } => "DeleteFile",
            Message::DeleteFileResponse { .. } => "DeleteFileResponse",
            Message::TruncateFile { .. } => "TruncateFile",
            Message::TruncateFileResponse { .. } => "TruncateFileResponse",
            Message::ListDirectory { .. } => "ListDirectory",
            Message::ListDirectoryResponse { .. } => "ListDirectoryResponse",
            Message::CreateDirectory { .. } => "CreateDirectory",
            Message::CreateDirectoryResponse { .. } => "CreateDirectoryResponse",
            Message::RemoveDirectory { .. } => "RemoveDirectory",
            Message::RemoveDirectoryResponse { .. } => "RemoveDirectoryResponse",
            Message::GetMetadata { .. } => "GetMetadata",
            Message::GetMetadataResponse { .. } => "GetMetadataResponse",
            Message::SetMetadata { .. } => "SetMetadata",
            Message::SetMetadataResponse { .. } => "SetMetadataResponse",
            Message::Rename { .. } => "Rename",
            Message::RenameResponse { .. } => "RenameResponse",
            Message::CreateSymlink { .. } => "CreateSymlink",
            Message::CreateSymlinkResponse { .. } => "CreateSymlinkResponse",
            Message::PathExists { .. } => "PathExists",
            Message::PathExistsResponse { .. } => "PathExistsResponse",
            Message::GetSpaceInfo { .. } => "GetSpaceInfo",
            Message::GetSpaceInfoResponse { .. } => "GetSpaceInfoResponse",
            Message::Ping { .. } => "Ping",
            Message::Pong { .. } => "Pong",
            Message::ConnectionClose { .. } => "ConnectionClose",
            Message::Error { .. } => "Error",
        }
    }
}

impl std::fmt::Display for ErrorCode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let name = match self {
            ErrorCode::AuthenticationFailed => "AuthenticationFailed",
            ErrorCode::InvalidCredentials => "InvalidCredentials",
            ErrorCode::SessionExpired => "SessionExpired",
            ErrorCode::AccessDenied => "AccessDenied",
            ErrorCode::PathNotAllowed => "PathNotAllowed",
            ErrorCode::InsufficientPermissions => "InsufficientPermissions",
            ErrorCode::FileNotFound => "FileNotFound",
            ErrorCode::DirectoryNotFound => "DirectoryNotFound",
            ErrorCode::PathAlreadyExists => "PathAlreadyExists",
            ErrorCode::InvalidPath => "InvalidPath",
            ErrorCode::DiskFull => "DiskFull",
            ErrorCode::ReadOnlyFileSystem => "ReadOnlyFileSystem",
            ErrorCode::NetworkError => "NetworkError",
            ErrorCode::ConnectionTimeout => "ConnectionTimeout",
            ErrorCode::MessageTooLarge => "MessageTooLarge",
            ErrorCode::InvalidMessage => "InvalidMessage",
            ErrorCode::InternalError => "InternalError",
            ErrorCode::NotImplemented => "NotImplemented",
            ErrorCode::ServiceUnavailable => "ServiceUnavailable",
        };
        write!(f, "{}", name)
    }
}

/// Utility for generating unique request IDs
pub fn generate_request_id() -> RequestId {
    Uuid::new_v4()
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_message_serialization() {
        let msg = Message::ReadFile {
            request_id: generate_request_id(),
            path: "/test/file.txt".to_string(),
            offset: 0,
            length: 1024,
        };
        
        let serialized = bincode::serialize(&msg).expect("Serialization failed");
        let deserialized: Message = bincode::deserialize(&serialized).expect("Deserialization failed");
        
        assert_eq!(msg.message_type(), deserialized.message_type());
        assert_eq!(msg.request_id(), deserialized.request_id());
    }
    
    #[test]
    fn test_request_response_matching() {
        let request_id = generate_request_id();
        let request = Message::ReadFile {
            request_id,
            path: "/test".to_string(),
            offset: 0,
            length: 100,
        };
        
        let response = Message::ReadFileResponse {
            request_id,
            success: true,
            data: Some("test data".as_bytes().to_vec()),
            bytes_read: 9,
            error: None,
        };
        
        assert_eq!(request.request_id(), response.request_id());
        assert!(!request.is_response());
        assert!(response.is_response());
    }
}
