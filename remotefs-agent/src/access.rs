use remotefs_common::{
    config::AccessConfig,
    error::{RemoteFsError, Result},
};
use crate::server::AccessControlStatistics;
use std::{
    collections::HashSet,
    path::{Path, PathBuf},
    sync::Arc,
};
use tokio::sync::RwLock;
use tracing::debug;

/// Access control manager that enforces security policies
#[derive(Clone)]
pub struct AccessControl {
    config: AccessConfig,
    stats: Arc<RwLock<AccessControlStatistics>>,
    allowed_paths: HashSet<PathBuf>,
    read_only_paths: HashSet<PathBuf>,
    denied_paths: HashSet<PathBuf>,
    allowed_extensions: HashSet<String>,
    denied_extensions: HashSet<String>,
}

impl AccessControl {
    /// Create a new access control manager
    pub fn new(config: &AccessConfig) -> Self {
        let allowed_paths: HashSet<PathBuf> = config.allowed_paths
            .iter()
            .map(|p| normalize_path(p))
            .collect();
            
        let read_only_paths: HashSet<PathBuf> = config.read_only_paths
            .iter()
            .map(|p| normalize_path(p))
            .collect();
            
        let denied_paths: HashSet<PathBuf> = config.denied_paths
            .iter()
            .map(|p| normalize_path(p))
            .collect();
            
        let allowed_extensions: HashSet<String> = config.allowed_extensions
            .iter()
            .map(|ext| ext.to_lowercase())
            .collect();
            
        let denied_extensions: HashSet<String> = config.denied_extensions
            .iter()
            .map(|ext| ext.to_lowercase())
            .collect();
        
        let stats = Arc::new(RwLock::new(AccessControlStatistics {
            allowed_requests: 0,
            denied_requests: 0,
            path_violations: 0,
            size_violations: 0,
        }));
        
        Self {
            config: config.clone(),
            stats,
            allowed_paths,
            read_only_paths,
            denied_paths,
            allowed_extensions,
            denied_extensions,
        }
    }
    
    /// Check if read access is allowed for a path
    pub async fn check_read_access(&self, path: &str) -> Result<()> {
        let result = self.check_path_access(path, AccessType::Read).await;
        self.update_stats(result.is_ok(), false, false).await;
        result
    }
    
    /// Check if write access is allowed for a path
    pub async fn check_write_access(&self, path: &str) -> Result<()> {
        let result = self.check_path_access(path, AccessType::Write).await;
        self.update_stats(result.is_ok(), false, false).await;
        result
    }
    
    /// Check if create access is allowed for a path
    pub async fn check_create_access(&self, path: &str) -> Result<()> {
        let result = self.check_path_access(path, AccessType::Create).await;
        self.update_stats(result.is_ok(), false, false).await;
        result
    }
    
    /// Check if delete access is allowed for a path
    pub async fn check_delete_access(&self, path: &str) -> Result<()> {
        let result = self.check_path_access(path, AccessType::Delete).await;
        self.update_stats(result.is_ok(), false, false).await;
        result
    }
    
    /// Check if a file size is within limits
    pub async fn check_file_size(&self, size: u64) -> Result<()> {
        if size > self.config.max_file_size {
            self.update_stats(false, false, true).await;
            return Err(RemoteFsError::Authorization(format!(
                "File size {} exceeds maximum allowed size {}",
                size,
                self.config.max_file_size
            )));
        }
        
        Ok(())
    }
    
    /// Get access control statistics
    pub async fn get_statistics(&self) -> AccessControlStatistics {
        self.stats.read().await.clone()
    }
    
    /// Check path access for a specific access type
    async fn check_path_access(&self, path: &str, access_type: AccessType) -> Result<()> {
        let path_buf = normalize_path(path);
        
        // Resolve symlinks if following is disabled
        let resolved_path = if self.config.follow_symlinks {
            path_buf.clone()
        } else {
            // Check if path contains symlinks
            if self.contains_symlink(&path_buf)? {
                return Err(RemoteFsError::Authorization(
                    "Symlinks are not allowed".to_string()
                ));
            }
            path_buf
        };
        
        // Check denied paths first (highest priority)
        if self.is_path_denied(&resolved_path) {
            debug!("Access denied - path in denied list: {}", path);
            return Err(RemoteFsError::Authorization(format!(
                "Access denied to path: {}",
                path
            )));
        }
        
        // Check if path is in allowed paths
        if !self.allowed_paths.is_empty() && !self.is_path_allowed(&resolved_path) {
            debug!("Access denied - path not in allowed list: {}", path);
            return Err(RemoteFsError::Authorization(format!(
                "Path not in allowed list: {}",
                path
            )));
        }
        
        // Check read-only restrictions for write operations
        if matches!(access_type, AccessType::Write | AccessType::Delete) {
            if self.is_path_read_only(&resolved_path) {
                debug!("Write access denied - path is read-only: {}", path);
                return Err(RemoteFsError::Authorization(format!(
                    "Path is read-only: {}",
                    path
                )));
            }
        }
        
        // Check file extension restrictions
        if let Some(extension) = resolved_path.extension().and_then(|e| e.to_str()) {
            let ext_lower = extension.to_lowercase();
            
            // Check denied extensions
            if !self.denied_extensions.is_empty() && self.denied_extensions.contains(&ext_lower) {
                debug!("Access denied - file extension denied: {}", extension);
                return Err(RemoteFsError::Authorization(format!(
                    "File extension '{}' is not allowed",
                    extension
                )));
            }
            
            // Check allowed extensions (if specified)
            if !self.allowed_extensions.is_empty() && !self.allowed_extensions.contains(&ext_lower) {
                debug!("Access denied - file extension not allowed: {}", extension);
                return Err(RemoteFsError::Authorization(format!(
                    "File extension '{}' is not in allowed list",
                    extension
                )));
            }
        }
        
        debug!("Access granted for {} operation on {}", access_type, path);
        Ok(())
    }
    
    /// Check if path is in denied list
    fn is_path_denied(&self, path: &Path) -> bool {
        self.denied_paths.iter().any(|denied| {
            path.starts_with(denied) || path == denied
        })
    }
    
    /// Check if path is in allowed list
    fn is_path_allowed(&self, path: &Path) -> bool {
        self.allowed_paths.iter().any(|allowed| {
            path.starts_with(allowed) || path == allowed
        })
    }
    
    /// Check if path is in read-only list
    fn is_path_read_only(&self, path: &Path) -> bool {
        self.read_only_paths.iter().any(|readonly| {
            path.starts_with(readonly) || path == readonly
        })
    }
    
    /// Check if path contains symlinks
    fn contains_symlink(&self, path: &Path) -> Result<bool> {
        let mut current = PathBuf::new();
        
        for component in path.components() {
            current.push(component);
            
            if current.exists() {
                let metadata = current.symlink_metadata()
                    .map_err(|e| RemoteFsError::FileSystem(format!(
                        "Failed to read symlink metadata: {}", e
                    )))?;
                
                if metadata.file_type().is_symlink() {
                    return Ok(true);
                }
            }
        }
        
        Ok(false)
    }
    
    /// Update access control statistics
    async fn update_stats(&self, allowed: bool, path_violation: bool, size_violation: bool) {
        let mut stats = self.stats.write().await;
        
        if allowed {
            stats.allowed_requests += 1;
        } else {
            stats.denied_requests += 1;
            
            if path_violation {
                stats.path_violations += 1;
            }
            
            if size_violation {
                stats.size_violations += 1;
            }
        }
    }
}

/// Type of access being requested
#[derive(Debug, Clone, Copy)]
enum AccessType {
    Read,
    Write,
    Create,
    Delete,
}

impl std::fmt::Display for AccessType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AccessType::Read => write!(f, "read"),
            AccessType::Write => write!(f, "write"),
            AccessType::Create => write!(f, "create"),
            AccessType::Delete => write!(f, "delete"),
        }
    }
}

/// Normalize a path by resolving it to an absolute path
fn normalize_path(path: &str) -> PathBuf {
    let path_buf = PathBuf::from(path);
    
    // Try to canonicalize if the path exists, otherwise just clean it up
    if path_buf.exists() {
        path_buf.canonicalize().unwrap_or_else(|_| clean_path(&path_buf))
    } else {
        clean_path(&path_buf)
    }
}

/// Clean up a path without requiring it to exist
fn clean_path(path: &Path) -> PathBuf {
    let mut components = Vec::new();
    
    for component in path.components() {
        match component {
            std::path::Component::CurDir => {
                // Skip current directory components
            }
            std::path::Component::ParentDir => {
                // Handle parent directory by popping the last component
                components.pop();
            }
            _ => {
                components.push(component);
            }
        }
    }
    
    components.iter().collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;
    
    fn create_test_access_config() -> AccessConfig {
        AccessConfig {
            allowed_paths: vec!["/tmp".to_string(), "/home/user".to_string()],
            read_only_paths: vec!["/home/user/readonly".to_string()],
            denied_paths: vec!["/etc".to_string(), "/root".to_string()],
            max_file_size: 1024 * 1024, // 1MB
            follow_symlinks: false,
            allowed_extensions: vec!["txt".to_string(), "md".to_string()],
            denied_extensions: vec!["exe".to_string(), "bat".to_string()],
        }
    }
    
    #[tokio::test]
    async fn test_path_access_allowed() {
        let config = create_test_access_config();
        let access_control = AccessControl::new(&config);
        
        // Test allowed path
        assert!(access_control.check_read_access("/tmp/test.txt").await.is_ok());
        assert!(access_control.check_read_access("/home/user/document.md").await.is_ok());
    }
    
    #[tokio::test]
    async fn test_path_access_denied() {
        let config = create_test_access_config();
        let access_control = AccessControl::new(&config);
        
        // Test denied path
        assert!(access_control.check_read_access("/etc/passwd").await.is_err());
        assert!(access_control.check_read_access("/root/secret").await.is_err());
    }
    
    #[tokio::test]
    async fn test_read_only_paths() {
        let config = create_test_access_config();
        let access_control = AccessControl::new(&config);
        
        // Read should be allowed
        assert!(access_control.check_read_access("/home/user/readonly/file.txt").await.is_ok());
        
        // Write should be denied
        assert!(access_control.check_write_access("/home/user/readonly/file.txt").await.is_err());
        assert!(access_control.check_delete_access("/home/user/readonly/file.txt").await.is_err());
    }
    
    #[tokio::test]
    async fn test_file_extension_filtering() {
        let config = create_test_access_config();
        let access_control = AccessControl::new(&config);
        
        // Allowed extensions
        assert!(access_control.check_read_access("/tmp/document.txt").await.is_ok());
        assert!(access_control.check_read_access("/tmp/readme.md").await.is_ok());
        
        // Denied extensions
        assert!(access_control.check_read_access("/tmp/malware.exe").await.is_err());
        assert!(access_control.check_read_access("/tmp/script.bat").await.is_err());
        
        // Extension not in allowed list
        assert!(access_control.check_read_access("/tmp/image.jpg").await.is_err());
    }
    
    #[tokio::test]
    async fn test_file_size_limits() {
        let config = create_test_access_config();
        let access_control = AccessControl::new(&config);
        
        // Within size limit
        assert!(access_control.check_file_size(1024).await.is_ok());
        assert!(access_control.check_file_size(1024 * 1024).await.is_ok());
        
        // Exceeds size limit
        assert!(access_control.check_file_size(1024 * 1024 + 1).await.is_err());
        assert!(access_control.check_file_size(10 * 1024 * 1024).await.is_err());
    }
    
    #[tokio::test]
    async fn test_statistics_tracking() {
        let config = create_test_access_config();
        let access_control = AccessControl::new(&config);
        
        // Perform some operations
        let _ = access_control.check_read_access("/tmp/test.txt").await;
        let _ = access_control.check_read_access("/etc/passwd").await;
        let _ = access_control.check_file_size(10 * 1024 * 1024).await;
        
        let stats = access_control.get_statistics().await;
        
        assert_eq!(stats.allowed_requests, 1);
        assert_eq!(stats.denied_requests, 2);
        assert_eq!(stats.path_violations, 1);
        assert_eq!(stats.size_violations, 1);
    }
    
    #[test]
    fn test_path_normalization() {
        assert_eq!(normalize_path("/tmp"), PathBuf::from("/tmp"));
        assert_eq!(clean_path(&PathBuf::from("/tmp/../etc")), PathBuf::from("/etc"));
        assert_eq!(clean_path(&PathBuf::from("/tmp/./file")), PathBuf::from("/tmp/file"));
        assert_eq!(clean_path(&PathBuf::from("/tmp/dir/../file")), PathBuf::from("/tmp/file"));
    }
    
    #[tokio::test]
    async fn test_symlink_detection() {
        let temp_dir = TempDir::new().unwrap();
        let temp_path = temp_dir.path();
        
        // Create a regular file
        let file_path = temp_path.join("regular_file.txt");
        fs::write(&file_path, "test content").unwrap();
        
        // Create a symlink
        let symlink_path = temp_path.join("symlink.txt");
        #[cfg(unix)]
        std::os::unix::fs::symlink(&file_path, &symlink_path).unwrap();
        
        let mut config = create_test_access_config();
        config.allowed_paths = vec![temp_path.to_string_lossy().to_string()];
        config.follow_symlinks = false;
        
        let access_control = AccessControl::new(&config);
        
        // Regular file should be accessible
        assert!(access_control.check_read_access(file_path.to_str().unwrap()).await.is_ok());
        
        // Symlink should be denied when follow_symlinks is false
        #[cfg(unix)]
        assert!(access_control.check_read_access(symlink_path.to_str().unwrap()).await.is_err());
    }
}
