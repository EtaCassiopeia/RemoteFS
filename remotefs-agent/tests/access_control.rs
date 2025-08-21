use std::path::Path;
use tokio;

mod common;
use common::*;
use remotefs_agent::access::AccessControl;
use remotefs_common::error::RemoteFsError;

#[tokio::test]
async fn test_access_control_creation() {
    setup_test_logging();
    let temp_dir = create_temp_dir();
    let config = create_test_config(temp_dir.path());
    
    let access_control = AccessControl::new(&config.access);
    let stats = access_control.get_statistics().await;
    
    assert_eq!(stats.allowed_requests, 0);
    assert_eq!(stats.denied_requests, 0);
}

#[tokio::test]
async fn test_allowed_path_access() {
    setup_test_logging();
    let temp_dir = create_temp_dir();
    create_test_directory_structure(temp_dir.path());
    let config = create_test_config(temp_dir.path());
    
    let access_control = AccessControl::new(&config.access);
    
    // Test allowed paths
    let allowed_path = temp_dir.path().join("allowed/test.txt").to_string_lossy().to_string();
    let result = access_control.check_read_access(&allowed_path).await;
    assert!(result.is_ok(), "Should allow access to allowed path");
    
    let nested_path = temp_dir.path().join("allowed/subdir1/nested.txt").to_string_lossy().to_string();
    let result = access_control.check_read_access(&nested_path).await;
    assert!(result.is_ok(), "Should allow access to nested allowed path");
}

#[tokio::test]
async fn test_denied_path_access() {
    setup_test_logging();
    let temp_dir = create_temp_dir();
    create_test_directory_structure(temp_dir.path());
    let config = create_test_config(temp_dir.path());
    
    let access_control = AccessControl::new(&config.access);
    
    // Test denied paths
    let denied_path = temp_dir.path().join("denied/secret.txt").to_string_lossy().to_string();
    let result = access_control.check_read_access(&denied_path).await;
    assert!(result.is_err(), "Should deny access to denied path");
    
    match result {
        Err(RemoteFsError::AccessDenied(msg)) => {
            assert!(msg.contains("denied"), "Error message should mention denial");
        },
        _ => panic!("Expected AccessDenied error"),
    }
}

#[tokio::test]
async fn test_read_only_path_access() {
    setup_test_logging();
    let temp_dir = create_temp_dir();
    create_test_directory_structure(temp_dir.path());
    let config = create_test_config(temp_dir.path());
    
    let access_control = AccessControl::new(&config.access);
    
    let readonly_path = temp_dir.path().join("readonly/readonly.txt").to_string_lossy().to_string();
    
    // Read access should be allowed
    let result = access_control.check_read_access(&readonly_path).await;
    assert!(result.is_ok(), "Should allow read access to read-only path");
    
    // Write access should be denied
    let result = access_control.check_write_access(&readonly_path).await;
    assert!(result.is_err(), "Should deny write access to read-only path");
    
    // Delete access should be denied
    let result = access_control.check_delete_access(&readonly_path).await;
    assert!(result.is_err(), "Should deny delete access to read-only path");
}

#[tokio::test]
async fn test_unauthorized_path_access() {
    setup_test_logging();
    let temp_dir = create_temp_dir();
    create_test_directory_structure(temp_dir.path());
    let config = create_test_config(temp_dir.path());
    
    let access_control = AccessControl::new(&config.access);
    
    // Test path not in allowed list
    let unauthorized_path = "/etc/passwd";
    let result = access_control.check_read_access(unauthorized_path).await;
    assert!(result.is_err(), "Should deny access to unauthorized path");
}

#[tokio::test]
async fn test_file_size_limits() {
    setup_test_logging();
    let temp_dir = create_temp_dir();
    let config = create_test_config(temp_dir.path());
    
    let access_control = AccessControl::new(&config.access);
    
    // Test within size limit
    let normal_size = 1024; // 1KB, well under 1MB limit
    let result = access_control.check_file_size(normal_size).await;
    assert!(result.is_ok(), "Should allow file within size limit");
    
    // Test exceeding size limit
    let large_size = 2 * 1024 * 1024; // 2MB, over 1MB limit
    let result = access_control.check_file_size(large_size).await;
    assert!(result.is_err(), "Should deny file exceeding size limit");
}

#[tokio::test]
async fn test_file_extension_filtering() {
    setup_test_logging();
    let temp_dir = create_temp_dir();
    create_test_directory_structure(temp_dir.path());
    let config = create_test_config(temp_dir.path());
    
    let access_control = AccessControl::new(&config.access);
    
    // Test allowed extension
    let pdf_path = temp_dir.path().join("allowed/document.pdf").to_string_lossy().to_string();
    let result = access_control.check_read_access(&pdf_path).await;
    assert!(result.is_ok(), "Should allow access to PDF file");
    
    // Test denied extension
    let exe_path = temp_dir.path().join("allowed/executable.exe").to_string_lossy().to_string();
    let result = access_control.check_read_access(&exe_path).await;
    assert!(result.is_err(), "Should deny access to EXE file");
}

#[tokio::test]
async fn test_access_statistics() {
    setup_test_logging();
    let temp_dir = create_temp_dir();
    create_test_directory_structure(temp_dir.path());
    let config = create_test_config(temp_dir.path());
    
    let access_control = AccessControl::new(&config.access);
    
    // Perform some access checks
    let allowed_path = temp_dir.path().join("allowed/test.txt").to_string_lossy().to_string();
    let denied_path = temp_dir.path().join("denied/secret.txt").to_string_lossy().to_string();
    
    // Allowed access
    let _ = access_control.check_read_access(&allowed_path).await;
    let _ = access_control.check_read_access(&allowed_path).await;
    
    // Denied access
    let _ = access_control.check_read_access(&denied_path).await;
    
    let stats = access_control.get_statistics().await;
    assert_eq!(stats.allowed_requests, 2, "Should have 2 allowed requests");
    assert_eq!(stats.denied_requests, 1, "Should have 1 denied request");
}

#[tokio::test] 
async fn test_create_access_validation() {
    setup_test_logging();
    let temp_dir = create_temp_dir();
    create_test_directory_structure(temp_dir.path());
    let config = create_test_config(temp_dir.path());
    
    let access_control = AccessControl::new(&config.access);
    
    // Test creating file in allowed directory
    let new_file_path = temp_dir.path().join("allowed/new_file.txt").to_string_lossy().to_string();
    let result = access_control.check_create_access(&new_file_path).await;
    assert!(result.is_ok(), "Should allow creating file in allowed directory");
    
    // Test creating file in denied directory
    let denied_file_path = temp_dir.path().join("denied/new_secret.txt").to_string_lossy().to_string();
    let result = access_control.check_create_access(&denied_file_path).await;
    assert!(result.is_err(), "Should deny creating file in denied directory");
    
    // Test creating file in read-only directory
    let readonly_file_path = temp_dir.path().join("readonly/new_readonly.txt").to_string_lossy().to_string();
    let result = access_control.check_create_access(&readonly_file_path).await;
    assert!(result.is_err(), "Should deny creating file in read-only directory");
}

#[tokio::test]
async fn test_write_access_validation() {
    setup_test_logging();
    let temp_dir = create_temp_dir();
    create_test_directory_structure(temp_dir.path());
    let config = create_test_config(temp_dir.path());
    
    let access_control = AccessControl::new(&config.access);
    
    // Test writing to allowed path
    let allowed_path = temp_dir.path().join("allowed/test.txt").to_string_lossy().to_string();
    let result = access_control.check_write_access(&allowed_path).await;
    assert!(result.is_ok(), "Should allow writing to allowed path");
    
    // Test writing to read-only path
    let readonly_path = temp_dir.path().join("readonly/readonly.txt").to_string_lossy().to_string();
    let result = access_control.check_write_access(&readonly_path).await;
    assert!(result.is_err(), "Should deny writing to read-only path");
}

#[tokio::test]
async fn test_delete_access_validation() {
    setup_test_logging();
    let temp_dir = create_temp_dir();
    create_test_directory_structure(temp_dir.path());
    let config = create_test_config(temp_dir.path());
    
    let access_control = AccessControl::new(&config.access);
    
    // Test deleting from allowed path
    let allowed_path = temp_dir.path().join("allowed/test.txt").to_string_lossy().to_string();
    let result = access_control.check_delete_access(&allowed_path).await;
    assert!(result.is_ok(), "Should allow deleting from allowed path");
    
    // Test deleting from read-only path
    let readonly_path = temp_dir.path().join("readonly/readonly.txt").to_string_lossy().to_string();
    let result = access_control.check_delete_access(&readonly_path).await;
    assert!(result.is_err(), "Should deny deleting from read-only path");
    
    // Test deleting from denied path
    let denied_path = temp_dir.path().join("denied/secret.txt").to_string_lossy().to_string();
    let result = access_control.check_delete_access(&denied_path).await;
    assert!(result.is_err(), "Should deny deleting from denied path");
}

#[tokio::test]
async fn test_path_normalization() {
    setup_test_logging();
    let temp_dir = create_temp_dir();
    create_test_directory_structure(temp_dir.path());
    let config = create_test_config(temp_dir.path());
    
    let access_control = AccessControl::new(&config.access);
    
    // Test path with .. components (should be normalized and still allowed if valid)
    let allowed_base = temp_dir.path().join("allowed").to_string_lossy().to_string();
    let normalized_path = format!("{}/subdir1/../test.txt", allowed_base);
    
    let result = access_control.check_read_access(&normalized_path).await;
    assert!(result.is_ok(), "Should allow access to normalized path within allowed directory");
}

#[tokio::test] 
async fn test_concurrent_access_checks() {
    setup_test_logging();
    let temp_dir = create_temp_dir();
    create_test_directory_structure(temp_dir.path());
    let config = create_test_config(temp_dir.path());
    
    let access_control = AccessControl::new(&config.access);
    let allowed_path = temp_dir.path().join("allowed/test.txt").to_string_lossy().to_string();
    
    // Run multiple concurrent access checks
    let tasks: Vec<_> = (0..10).map(|_| {
        let access_control = access_control.clone();
        let path = allowed_path.clone();
        tokio::spawn(async move {
            access_control.check_read_access(&path).await
        })
    }).collect();
    
    let results: Vec<_> = futures::future::join_all(tasks).await;
    
    // All should succeed
    for result in results {
        assert!(result.unwrap().is_ok(), "All concurrent access checks should succeed");
    }
    
    // Verify statistics
    let stats = access_control.get_statistics().await;
    assert_eq!(stats.allowed_requests, 10, "Should have 10 allowed requests");
}
