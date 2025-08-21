use remotefs_common::config::ClientConfig;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Configuration for FUSE filesystem operations
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FuseConfig {
    /// Client configuration for connecting to RemoteFS
    pub client: ClientConfig,
    /// Mount points configuration
    pub mounts: Vec<MountPointConfig>,
    /// Global FUSE options
    pub fuse_options: MountOptions,
    /// Cache configuration
    pub cache: CacheConfig,
}

/// Configuration for a single mount point
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MountPointConfig {
    /// Local mount point directory
    pub mount_path: PathBuf,
    /// Remote path to mount
    pub remote_path: PathBuf,
    /// Agent ID to connect to
    pub agent_id: String,
    /// Mount-specific options (overrides global options)
    pub options: Option<MountOptions>,
}

/// FUSE mount options
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MountOptions {
    /// User ID for mounted files
    pub uid: Option<u32>,
    /// Group ID for mounted files
    pub gid: Option<u32>,
    /// Allow other users to access the mount
    pub allow_other: bool,
    /// Allow root to access the mount
    pub allow_root: bool,
    /// Filesystem name in mount table
    pub fsname: Option<String>,
    /// Filesystem subtype
    pub subtype: Option<String>,
    /// Enable automatic caching
    pub auto_cache: bool,
    /// Automatically unmount on exit
    pub auto_unmount: bool,
    /// Mount read-only
    pub read_only: bool,
    /// Allow executable files
    pub exec: bool,
    /// Allow SUID files
    pub suid: bool,
    /// Allow device files
    pub dev: bool,
    /// Update access times
    pub atime: bool,
    /// Use default permissions
    pub default_permissions: bool,
}

impl Default for MountOptions {
    fn default() -> Self {
        Self {
            uid: None,
            gid: None,
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
}

/// Cache configuration for FUSE filesystem
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CacheConfig {
    /// Enable attribute caching
    pub attr_cache: bool,
    /// Attribute cache TTL in seconds
    pub attr_timeout: u64,
    /// Entry cache TTL in seconds
    pub entry_timeout: u64,
    /// Enable negative caching (cache failed lookups)
    pub negative_timeout: u64,
    /// Maximum number of cached entries
    pub max_entries: usize,
    /// Enable write-through cache
    pub write_through: bool,
    /// Write cache size in bytes
    pub write_cache_size: usize,
}

impl Default for CacheConfig {
    fn default() -> Self {
        Self {
            attr_cache: true,
            attr_timeout: 10,
            entry_timeout: 10,
            negative_timeout: 2,
            max_entries: 10000,
            write_through: true,
            write_cache_size: 4 * 1024 * 1024, // 4MB
        }
    }
}

impl FuseConfig {
    /// Create a default FUSE configuration
    pub fn default_with_client(client_config: ClientConfig) -> Self {
        Self {
            client: client_config,
            mounts: Vec::new(),
            fuse_options: MountOptions::default(),
            cache: CacheConfig::default(),
        }
    }

    /// Add a mount point to the configuration
    pub fn add_mount_point(
        &mut self,
        mount_path: PathBuf,
        remote_path: PathBuf,
        agent_id: String,
        options: Option<MountOptions>,
    ) {
        self.mounts.push(MountPointConfig {
            mount_path,
            remote_path,
            agent_id,
            options,
        });
    }

    /// Validate the configuration
    pub fn validate(&self) -> Result<(), String> {
        // Validate client configuration
        if self.client.agents.is_empty() {
            return Err("No agents configured in client".to_string());
        }

        // Validate mount points
        if self.mounts.is_empty() {
            return Err("No mount points configured".to_string());
        }

        for (i, mount) in self.mounts.iter().enumerate() {
            // Check if mount path is absolute
            if !mount.mount_path.is_absolute() {
                return Err(format!("Mount point {} path must be absolute", i));
            }

            // Check if remote path is absolute
            if !mount.remote_path.is_absolute() {
                return Err(format!("Mount point {} remote path must be absolute", i));
            }

            // Check if agent exists in client configuration
            let agent_exists = self.client.agents.iter().any(|agent| agent.id == mount.agent_id);
            if !agent_exists {
                return Err(format!(
                    "Mount point {} references unknown agent: {}",
                    i, mount.agent_id
                ));
            }
        }

        // Validate cache configuration
        if self.cache.max_entries == 0 {
            return Err("Cache max_entries must be greater than 0".to_string());
        }

        if self.cache.write_cache_size == 0 {
            return Err("Write cache size must be greater than 0".to_string());
        }

        Ok(())
    }

    /// Get the effective mount options for a mount point
    pub fn get_effective_options(&self, mount_index: usize) -> Option<MountOptions> {
        self.mounts.get(mount_index).map(|mount| {
            mount.options.clone().unwrap_or_else(|| self.fuse_options.clone())
        })
    }
}

/// Load FUSE configuration from a file
pub fn load_fuse_config<P: AsRef<std::path::Path>>(path: P) -> Result<FuseConfig, Box<dyn std::error::Error>> {
    let content = std::fs::read_to_string(path)?;
    let config: FuseConfig = toml::from_str(&content)?;
    config.validate().map_err(|e| format!("Configuration validation failed: {}"))?;
    Ok(config)
}

/// Save FUSE configuration to a file
pub fn save_fuse_config<P: AsRef<std::path::Path>>(
    config: &FuseConfig,
    path: P,
) -> Result<(), Box<dyn std::error::Error>> {
    config.validate().map_err(|e| format!("Configuration validation failed: {}"))?;
    let content = toml::to_string_pretty(config)?;
    
    // Create parent directory if it doesn't exist
    if let Some(parent) = path.as_ref().parent() {
        std::fs::create_dir_all(parent)?;
    }
    
    std::fs::write(path, content)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use remotefs_common::config::{AgentConfig, ClientConfig as CommonClientConfig};
    use tempfile::TempDir;

    fn create_test_client_config() -> ClientConfig {
        ClientConfig {
            client_id: "test-client".to_string(),
            agents: vec![
                AgentConfig {
                    id: "agent1".to_string(),
                    url: "ws://localhost:8080".to_string(),
                    auth: None,
                    weight: 1,
                    enabled: true,
                },
            ],
            client: remotefs_common::config::ClientBehaviorConfig::default(),
            connection: remotefs_common::config::ConnectionConfig::default(),
            auth: None,
            logging: remotefs_common::config::LoggingConfig::default(),
        }
    }

    #[test]
    fn test_default_mount_options() {
        let options = MountOptions::default();
        assert_eq!(options.fsname, Some("remotefs".to_string()));
        assert_eq!(options.subtype, Some("remotefs".to_string()));
        assert!(options.auto_cache);
        assert!(options.auto_unmount);
        assert!(!options.read_only);
    }

    #[test]
    fn test_default_cache_config() {
        let cache = CacheConfig::default();
        assert!(cache.attr_cache);
        assert_eq!(cache.attr_timeout, 10);
        assert_eq!(cache.entry_timeout, 10);
        assert!(cache.write_through);
        assert!(cache.max_entries > 0);
    }

    #[test]
    fn test_fuse_config_creation() {
        let client_config = create_test_client_config();
        let fuse_config = FuseConfig::default_with_client(client_config.clone());
        
        assert_eq!(fuse_config.client.client_id, client_config.client_id);
        assert!(fuse_config.mounts.is_empty());
    }

    #[test]
    fn test_add_mount_point() {
        let client_config = create_test_client_config();
        let mut fuse_config = FuseConfig::default_with_client(client_config);
        
        fuse_config.add_mount_point(
            PathBuf::from("/mnt/remote"),
            PathBuf::from("/remote/path"),
            "agent1".to_string(),
            None,
        );
        
        assert_eq!(fuse_config.mounts.len(), 1);
        assert_eq!(fuse_config.mounts[0].mount_path, PathBuf::from("/mnt/remote"));
        assert_eq!(fuse_config.mounts[0].remote_path, PathBuf::from("/remote/path"));
        assert_eq!(fuse_config.mounts[0].agent_id, "agent1");
    }

    #[test]
    fn test_config_validation() {
        let client_config = create_test_client_config();
        let mut fuse_config = FuseConfig::default_with_client(client_config);
        
        // Should fail without mount points
        assert!(fuse_config.validate().is_err());
        
        // Add valid mount point
        fuse_config.add_mount_point(
            PathBuf::from("/mnt/remote"),
            PathBuf::from("/remote/path"),
            "agent1".to_string(),
            None,
        );
        
        // Should pass with valid mount point
        assert!(fuse_config.validate().is_ok());
        
        // Should fail with non-existent agent
        fuse_config.add_mount_point(
            PathBuf::from("/mnt/remote2"),
            PathBuf::from("/remote/path2"),
            "nonexistent".to_string(),
            None,
        );
        
        assert!(fuse_config.validate().is_err());
    }

    #[test]
    fn test_effective_options() {
        let client_config = create_test_client_config();
        let mut fuse_config = FuseConfig::default_with_client(client_config);
        
        // Add mount without specific options
        fuse_config.add_mount_point(
            PathBuf::from("/mnt/remote1"),
            PathBuf::from("/remote/path1"),
            "agent1".to_string(),
            None,
        );
        
        // Add mount with specific options
        let mut custom_options = MountOptions::default();
        custom_options.read_only = true;
        fuse_config.add_mount_point(
            PathBuf::from("/mnt/remote2"),
            PathBuf::from("/remote/path2"),
            "agent1".to_string(),
            Some(custom_options.clone()),
        );
        
        // First mount should use global options
        let options1 = fuse_config.get_effective_options(0).unwrap();
        assert!(!options1.read_only);
        
        // Second mount should use custom options
        let options2 = fuse_config.get_effective_options(1).unwrap();
        assert!(options2.read_only);
    }

    #[test]
    fn test_config_serialization() {
        let client_config = create_test_client_config();
        let mut fuse_config = FuseConfig::default_with_client(client_config);
        
        fuse_config.add_mount_point(
            PathBuf::from("/mnt/remote"),
            PathBuf::from("/remote/path"),
            "agent1".to_string(),
            None,
        );
        
        // Test TOML serialization
        let toml = toml::to_string_pretty(&fuse_config).unwrap();
        assert!(toml.contains("mount_path"));
        assert!(toml.contains("remote_path"));
        assert!(toml.contains("agent_id"));
        
        // Test deserialization
        let deserialized: FuseConfig = toml::from_str(&toml).unwrap();
        assert_eq!(deserialized.mounts.len(), fuse_config.mounts.len());
        assert_eq!(deserialized.mounts[0].agent_id, fuse_config.mounts[0].agent_id);
    }

    #[test]
    fn test_save_and_load_config() {
        let temp_dir = TempDir::new().unwrap();
        let config_path = temp_dir.path().join("fuse_config.toml");
        
        let client_config = create_test_client_config();
        let mut fuse_config = FuseConfig::default_with_client(client_config);
        
        fuse_config.add_mount_point(
            PathBuf::from("/mnt/remote"),
            PathBuf::from("/remote/path"),
            "agent1".to_string(),
            None,
        );
        
        // Save config
        save_fuse_config(&fuse_config, &config_path).unwrap();
        assert!(config_path.exists());
        
        // Load config
        let loaded_config = load_fuse_config(&config_path).unwrap();
        assert_eq!(loaded_config.mounts.len(), 1);
        assert_eq!(loaded_config.mounts[0].agent_id, "agent1");
    }
}
