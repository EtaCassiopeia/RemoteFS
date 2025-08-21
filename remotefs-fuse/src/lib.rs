//! RemoteFS FUSE Library
//! 
//! This library provides FUSE (Filesystem in Userspace) integration for RemoteFS,
//! allowing remote filesystems to be mounted as if they were local directories.

pub mod filesystem;
pub mod mount;
pub mod config;
pub mod cache;

// Re-export commonly used types
pub use filesystem::RemoteFsFilesystem;
pub use mount::{MountPoint, mount_filesystem, unmount_filesystem};
pub use config::{FuseConfig, MountOptions};

use remotefs_common::error::{RemoteFsError, Result};
use std::path::PathBuf;

/// Version information
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Default mount options for RemoteFS FUSE mounts
pub fn default_mount_options() -> MountOptions {
    MountOptions {
        uid: Some(unsafe { libc::getuid() }),
        gid: Some(unsafe { libc::getgid() }),
        allow_other: false,
        allow_root: false,
        fsname: Some("remotefs".to_string()),
        subtype: Some("remotefs".to_string()),
        auto_cache: true,
        auto_unmount: true,
        read_only: false,
        exec: true,
        suid: false,
        dev: false,
        atime: true,
        default_permissions: true,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_version() {
        assert!(!VERSION.is_empty());
    }

    #[test]
    fn test_default_mount_options() {
        let options = default_mount_options();
        assert!(options.uid.is_some());
        assert!(options.gid.is_some());
        assert_eq!(options.fsname, Some("remotefs".to_string()));
        assert_eq!(options.subtype, Some("remotefs".to_string()));
    }
}
