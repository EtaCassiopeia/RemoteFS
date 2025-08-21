use std::path::{Path, PathBuf};
use std::fs;
use std::sync::Arc;
use tempfile::TempDir;
use remotefs_common::{
    config::{AgentConfig, AccessConfig, SecurityConfig, NetworkConfig, LoggingConfig, PerformanceConfig},
    defaults,
};
use remotefs_agent::access::AccessControl;

/// Create a temporary directory for tests
pub fn create_temp_dir() -> TempDir {
    TempDir::new().expect("Failed to create temp directory")
}

/// Create a test file with content
pub fn create_test_file<P: AsRef<Path>>(path: P, content: &str) {
    fs::write(path, content).expect("Failed to create test file");
}

/// Create a test directory structure
pub fn create_test_directory_structure(base: &Path) {
    let dirs = [
        "allowed/subdir1",
        "allowed/subdir2", 
        "readonly",
        "denied",
        "temp",
    ];
    
    for dir in &dirs {
        let dir_path = base.join(dir);
        fs::create_dir_all(&dir_path).expect("Failed to create test directory");
    }
    
    // Create test files
    create_test_file(base.join("allowed/test.txt"), "test content");
    create_test_file(base.join("allowed/subdir1/nested.txt"), "nested content");
    create_test_file(base.join("readonly/readonly.txt"), "readonly content");
    create_test_file(base.join("denied/secret.txt"), "secret content");
    create_test_file(base.join("temp/temp.txt"), "temp content");
    
    // Create files with different extensions
    create_test_file(base.join("allowed/document.pdf"), "pdf content");
    create_test_file(base.join("allowed/script.sh"), "#!/bin/bash\necho 'test'");
    create_test_file(base.join("allowed/executable.exe"), "fake exe");
    create_test_file(base.join("allowed/image.jpg"), "fake jpg");
}

/// Create a test agent configuration
pub fn create_test_config(temp_dir: &Path) -> AgentConfig {
    AgentConfig {
        agent_id: "test-agent".to_string(),
        relay_url: "ws://localhost:8080/ws".to_string(),
        access: AccessConfig {
            allowed_paths: vec![
                temp_dir.join("allowed").to_string_lossy().to_string(),
                temp_dir.join("temp").to_string_lossy().to_string(),
            ],
            read_only_paths: vec![
                temp_dir.join("readonly").to_string_lossy().to_string(),
            ],
            denied_paths: vec![
                temp_dir.join("denied").to_string_lossy().to_string(),
            ],
            max_file_size: 1024 * 1024, // 1MB
            follow_symlinks: true,
            allowed_extensions: vec![],
            denied_extensions: vec!["exe".to_string(), "bat".to_string()],
        },
        security: SecurityConfig {
            key_file: temp_dir.join("agent.key"),
            cert_file: temp_dir.join("agent.crt"),
            enable_tls: false,
            verify_certs: false,
            session_timeout: 3600,
            enable_auth: false,
            allowed_clients: vec![],
        },
        network: NetworkConfig::default(),
        logging: LoggingConfig {
            level: "debug".to_string(),
            format: "plain".to_string(),
            file: None,
            max_file_size: 100,
            max_files: 5,
            enable_access_log: false,
            access_log_file: None,
        },
        performance: PerformanceConfig {
            worker_threads: 2,
            io_buffer_size: 1024,
            async_io: true,
            fs_cache_size: 64,
            enable_prefetch: false,
            prefetch_window: 4,
        },
    }
}

/// Create a test access control instance
pub fn create_test_access_control(config: &AccessConfig) -> Arc<AccessControl> {
    Arc::new(AccessControl::new(config))
}

/// Assert that a path exists
pub fn assert_path_exists<P: AsRef<Path>>(path: P) {
    assert!(path.as_ref().exists(), "Path should exist: {}", path.as_ref().display());
}

/// Assert that a path does not exist
pub fn assert_path_not_exists<P: AsRef<Path>>(path: P) {
    assert!(!path.as_ref().exists(), "Path should not exist: {}", path.as_ref().display());
}

/// Assert that a file contains specific content
pub fn assert_file_content<P: AsRef<Path>>(path: P, expected_content: &str) {
    let actual_content = fs::read_to_string(path.as_ref())
        .expect(&format!("Failed to read file: {}", path.as_ref().display()));
    assert_eq!(actual_content, expected_content, "File content mismatch");
}

/// Assert that a directory contains specific entries
pub fn assert_directory_contains<P: AsRef<Path>>(dir_path: P, expected_entries: &[&str]) {
    let dir_path = dir_path.as_ref();
    let entries: Vec<String> = fs::read_dir(dir_path)
        .expect(&format!("Failed to read directory: {}", dir_path.display()))
        .map(|entry| {
            entry.expect("Failed to read directory entry")
                .file_name()
                .to_string_lossy()
                .to_string()
        })
        .collect();
    
    for expected in expected_entries {
        assert!(
            entries.contains(&expected.to_string()),
            "Directory {} should contain entry: {}. Found: {:?}",
            dir_path.display(),
            expected,
            entries
        );
    }
}

/// Create a large test file for size limit testing
pub fn create_large_test_file<P: AsRef<Path>>(path: P, size_bytes: usize) {
    let content = "x".repeat(size_bytes);
    fs::write(path, content).expect("Failed to create large test file");
}

/// Setup test environment with logging
pub fn setup_test_logging() {
    use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};
    
    let _ = tracing_subscriber::registry()
        .with(EnvFilter::try_new("debug").unwrap_or_else(|_| EnvFilter::new("info")))
        .with(tracing_subscriber::fmt::layer().with_test_writer())
        .try_init();
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_create_temp_dir() {
        let temp_dir = create_temp_dir();
        assert!(temp_dir.path().exists());
        assert!(temp_dir.path().is_dir());
    }
    
    #[test]
    fn test_create_test_file() {
        let temp_dir = create_temp_dir();
        let file_path = temp_dir.path().join("test.txt");
        let content = "test content";
        
        create_test_file(&file_path, content);
        assert_path_exists(&file_path);
        assert_file_content(&file_path, content);
    }
    
    #[test]
    fn test_create_test_directory_structure() {
        let temp_dir = create_temp_dir();
        create_test_directory_structure(temp_dir.path());
        
        // Verify directories exist
        assert_path_exists(temp_dir.path().join("allowed"));
        assert_path_exists(temp_dir.path().join("allowed/subdir1"));
        assert_path_exists(temp_dir.path().join("readonly"));
        assert_path_exists(temp_dir.path().join("denied"));
        
        // Verify files exist
        assert_path_exists(temp_dir.path().join("allowed/test.txt"));
        assert_path_exists(temp_dir.path().join("allowed/subdir1/nested.txt"));
        assert_path_exists(temp_dir.path().join("readonly/readonly.txt"));
        
        // Verify file contents
        assert_file_content(temp_dir.path().join("allowed/test.txt"), "test content");
        assert_file_content(temp_dir.path().join("allowed/subdir1/nested.txt"), "nested content");
    }
    
    #[test]
    fn test_create_test_config() {
        let temp_dir = create_temp_dir();
        let config = create_test_config(temp_dir.path());
        
        assert_eq!(config.agent_id, "test-agent");
        assert_eq!(config.relay_url, "ws://localhost:8080/ws");
        assert!(!config.access.allowed_paths.is_empty());
        assert!(!config.access.read_only_paths.is_empty());
        assert!(!config.access.denied_paths.is_empty());
        assert_eq!(config.access.max_file_size, 1024 * 1024);
    }
}
