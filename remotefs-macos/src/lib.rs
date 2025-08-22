pub mod nfs_filesystem;
pub mod server;
pub mod config;
pub mod cli;

pub use nfs_filesystem::RemoteNfsFilesystem;
pub use server::RemoteNfsServer;
pub use config::MacOSConfig;

use remotefs_common::error::RemoteFsError;

pub type Result<T> = std::result::Result<T, RemoteFsError>;
