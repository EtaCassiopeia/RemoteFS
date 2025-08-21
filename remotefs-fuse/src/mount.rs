use crate::{filesystem::RemoteFsFilesystem, config::{FuseConfig, MountOptions}};
use fuse3::{MountOptions as Fuse3MountOptions, Session, Result};
use remotefs_client::RemoteFsClient;
use remotefs_common::error::{RemoteFsError, Result as RemoteFsResult};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::sync::Mutex;
use tracing::{debug, error, info, warn};

/// Represents an active FUSE mount point
#[derive(Debug, Clone)]
pub struct MountPoint {
    /// Local mount point directory
    pub mount_path: PathBuf,
    /// Remote path being mounted
    pub remote_path: PathBuf,
    /// Mount options used
    pub options: MountOptions,
    /// Whether the mount is read-only
    pub read_only: bool,
}

impl MountPoint {
    /// Create a new mount point definition
    pub fn new<P: AsRef<Path>>(
        mount_path: P,
        remote_path: P,
        options: MountOptions,
    ) -> Self {
        Self {
            mount_path: mount_path.as_ref().to_path_buf(),
            remote_path: remote_path.as_ref().to_path_buf(),
            read_only: options.read_only,
            options,
        }
    }

    /// Check if the mount point directory exists and is a directory
    pub fn validate(&self) -> RemoteFsResult<()> {
        if !self.mount_path.exists() {
            return Err(RemoteFsError::NotFound(format!(
                "Mount point does not exist: {}",
                self.mount_path.display()
            )));
        }

        if !self.mount_path.is_dir() {
            return Err(RemoteFsError::InvalidInput(format!(
                "Mount point is not a directory: {}",
                self.mount_path.display()
            )));
        }

        // Check if directory is empty (recommended for mounting)
        match std::fs::read_dir(&self.mount_path) {
            Ok(mut entries) => {
                if entries.next().is_some() {
                    warn!(
                        "Mount point directory is not empty: {}",
                        self.mount_path.display()
                    );
                }
            }
            Err(e) => {
                warn!(
                    "Could not check mount point directory {}: {}",
                    self.mount_path.display(),
                    e
                );
            }
        }

        Ok(())
    }

    /// Check if the mount point appears to be mounted
    pub fn is_mounted(&self) -> bool {
        // On Unix systems, we can check /proc/mounts or use other methods
        // For now, we'll use a simple heuristic
        #[cfg(unix)]
        {
            use std::process::Command;
            
            let output = Command::new("mount")
                .output()
                .unwrap_or_else(|_| std::process::Output {
                    status: std::process::ExitStatus::from_raw(1),
                    stdout: Vec::new(),
                    stderr: Vec::new(),
                });
                
            if output.status.success() {
                let mount_output = String::from_utf8_lossy(&output.stdout);
                return mount_output
                    .lines()
                    .any(|line| line.contains(&self.mount_path.to_string_lossy().to_string()));
            }
        }
        
        false
    }
}

/// Active FUSE session handle
pub struct FuseSession {
    session: Arc<Mutex<Option<Session<RemoteFsFilesystem>>>>,
    mount_point: MountPoint,
}

impl FuseSession {
    /// Create a new FUSE session
    fn new(session: Session<RemoteFsFilesystem>, mount_point: MountPoint) -> Self {
        Self {
            session: Arc::new(Mutex::new(Some(session))),
            mount_point,
        }
    }

    /// Get the mount point information
    pub fn mount_point(&self) -> &MountPoint {
        &self.mount_point
    }

    /// Check if the session is still active
    pub async fn is_active(&self) -> bool {
        let session = self.session.lock().await;
        session.is_some()
    }

    /// Unmount the filesystem and destroy the session
    pub async fn unmount(self) -> RemoteFsResult<()> {
        let mut session = self.session.lock().await;
        if let Some(fuse_session) = session.take() {
            debug!("Unmounting {}", self.mount_point.mount_path.display());
            
            // The session will be automatically unmounted when dropped
            drop(fuse_session);
            
            info!("Successfully unmounted {}", self.mount_point.mount_path.display());
            Ok(())
        } else {
            warn!("Attempted to unmount already inactive session");
            Ok(())
        }
    }
}

/// Mount a remote filesystem using FUSE
pub async fn mount_filesystem(
    client: Arc<RemoteFsClient>,
    mount_point: MountPoint,
) -> RemoteFsResult<FuseSession> {
    info!(
        "Mounting remote path {} at {}",
        mount_point.remote_path.display(),
        mount_point.mount_path.display()
    );

    // Validate mount point
    mount_point.validate()?;

    // Check if already mounted
    if mount_point.is_mounted() {
        return Err(RemoteFsError::AlreadyExists(format!(
            "Mount point is already mounted: {}",
            mount_point.mount_path.display()
        )));
    }

    // Create the filesystem
    let filesystem = RemoteFsFilesystem::new(
        client,
        mount_point.mount_path.clone(),
        mount_point.remote_path.clone(),
    );

    // Convert our mount options to fuse3 mount options
    let fuse_options = mount_options_to_fuse3(&mount_point.options)?;

    // Create and start the FUSE session
    let session = Session::new(fuse_options)
        .mount_with_unprivileged(filesystem, &mount_point.mount_path)
        .await
        .map_err(|e| {
            error!("Failed to mount filesystem: {}", e);
            RemoteFsError::Io(std::io::Error::new(
                std::io::ErrorKind::Other,
                format!("FUSE mount failed: {}", e),
            ))
        })?;

    info!(
        "Successfully mounted {} at {}",
        mount_point.remote_path.display(),
        mount_point.mount_path.display()
    );

    Ok(FuseSession::new(session, mount_point))
}

/// Unmount a filesystem by path
pub async fn unmount_filesystem<P: AsRef<Path>>(mount_path: P) -> RemoteFsResult<()> {
    let mount_path = mount_path.as_ref();
    info!("Unmounting filesystem at {}", mount_path.display());

    #[cfg(unix)]
    {
        use std::process::Command;

        // Try to unmount using fusermount
        let result = Command::new("fusermount")
            .arg("-u")
            .arg(mount_path)
            .output();

        match result {
            Ok(output) => {
                if output.status.success() {
                    info!("Successfully unmounted {}", mount_path.display());
                    Ok(())
                } else {
                    let error_msg = String::from_utf8_lossy(&output.stderr);
                    error!("Failed to unmount {}: {}", mount_path.display(), error_msg);
                    Err(RemoteFsError::Io(std::io::Error::new(
                        std::io::ErrorKind::Other,
                        format!("Unmount failed: {}", error_msg),
                    )))
                }
            }
            Err(e) => {
                error!("Failed to execute fusermount: {}", e);
                Err(RemoteFsError::Io(e))
            }
        }
    }

    #[cfg(not(unix))]
    {
        error!("Unmount not implemented for this platform");
        Err(RemoteFsError::Configuration(
            "Unmount not supported on this platform".to_string(),
        ))
    }
}

/// Convert our MountOptions to fuse3 MountOptions
fn mount_options_to_fuse3(options: &MountOptions) -> RemoteFsResult<Fuse3MountOptions> {
    let mut fuse_opts = Fuse3MountOptions::default();

    // Set basic options
    if let Some(uid) = options.uid {
        fuse_opts = fuse_opts.uid(uid);
    }
    if let Some(gid) = options.gid {
        fuse_opts = fuse_opts.gid(gid);
    }

    // Set filesystem name and subtype
    if let Some(ref fsname) = options.fsname {
        fuse_opts = fuse_opts.fsname(fsname);
    }
    if let Some(ref subtype) = options.subtype {
        fuse_opts = fuse_opts.subtype(subtype);
    }

    // Set permission and access options
    if options.allow_other {
        fuse_opts = fuse_opts.allow_other(true);
    }
    if options.allow_root {
        fuse_opts = fuse_opts.allow_root(true);
    }
    if options.default_permissions {
        fuse_opts = fuse_opts.default_permissions(true);
    }

    // Set filesystem behavior options
    if options.read_only {
        fuse_opts = fuse_opts.read_only(true);
    }
    if !options.exec {
        fuse_opts = fuse_opts.noexec(true);
    }
    if !options.suid {
        fuse_opts = fuse_opts.nosuid(true);
    }
    if !options.dev {
        fuse_opts = fuse_opts.nodev(true);
    }
    if !options.atime {
        fuse_opts = fuse_opts.noatime(true);
    }

    // Set caching options
    if options.auto_cache {
        fuse_opts = fuse_opts.auto_cache(true);
    }
    if options.auto_unmount {
        fuse_opts = fuse_opts.auto_unmount(true);
    }

    Ok(fuse_opts)
}

/// Check if FUSE is available on the system
pub fn check_fuse_availability() -> RemoteFsResult<()> {
    #[cfg(unix)]
    {
        use std::path::Path;

        // Check if /dev/fuse exists
        if !Path::new("/dev/fuse").exists() {
            return Err(RemoteFsError::Configuration(
                "FUSE not available: /dev/fuse not found. Please install FUSE and load the kernel module.".to_string(),
            ));
        }

        // Check if fusermount is available
        match std::process::Command::new("fusermount").arg("--version").output() {
            Ok(output) => {
                if !output.status.success() {
                    warn!("fusermount found but returned error");
                }
            }
            Err(_) => {
                warn!("fusermount not found in PATH. Some functionality may be limited.");
            }
        }

        // Check permissions on /dev/fuse
        match std::fs::metadata("/dev/fuse") {
            Ok(metadata) => {
                use std::os::unix::fs::MetadataExt;
                let mode = metadata.mode();
                debug!("FUSE device permissions: {:o}", mode);
                
                // Basic check - in practice, permissions might be more complex
                if mode & 0o006 == 0 {
                    warn!("FUSE device may not be accessible to current user");
                }
            }
            Err(e) => {
                warn!("Could not check FUSE device permissions: {}", e);
            }
        }

        Ok(())
    }

    #[cfg(not(unix))]
    {
        Err(RemoteFsError::Configuration(
            "FUSE not supported on this platform".to_string(),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_mount_point_creation() {
        let temp_dir = TempDir::new().unwrap();
        let mount_point = MountPoint::new(
            temp_dir.path(),
            "/remote/path",
            crate::default_mount_options(),
        );

        assert_eq!(mount_point.mount_path, temp_dir.path());
        assert_eq!(mount_point.remote_path, PathBuf::from("/remote/path"));
        assert!(!mount_point.read_only);
    }

    #[test]
    fn test_mount_point_validation() {
        let temp_dir = TempDir::new().unwrap();
        let mount_point = MountPoint::new(
            temp_dir.path(),
            "/remote/path",
            crate::default_mount_options(),
        );

        // Should validate successfully for existing directory
        assert!(mount_point.validate().is_ok());

        // Should fail for non-existent directory
        let bad_mount_point = MountPoint::new(
            "/non/existent/path",
            "/remote/path",
            crate::default_mount_options(),
        );
        assert!(bad_mount_point.validate().is_err());
    }

    #[tokio::test]
    async fn test_check_fuse_availability() {
        // This test will pass or fail depending on the system
        // In a real test environment, you might want to mock this
        let result = check_fuse_availability();
        match result {
            Ok(_) => println!("FUSE is available"),
            Err(e) => println!("FUSE not available: {}", e),
        }
    }

    #[test]
    fn test_mount_options_conversion() {
        let options = crate::default_mount_options();
        let fuse_options = mount_options_to_fuse3(&options);
        assert!(fuse_options.is_ok());
    }
}
