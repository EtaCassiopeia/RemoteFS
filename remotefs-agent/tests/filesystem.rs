use std::sync::Arc;
use uuid::Uuid;
use tokio;

mod common;
use common::*;
use remotefs_agent::filesystem::FilesystemHandler;
use remotefs_common::config::PerformanceConfig;

#[tokio::test]
async fn test_filesystem_handler_creation() {
    setup_test_logging();
    let temp_dir = create_temp_dir();
    let config = create_test_config(temp_dir.path());
    let access_control = create_test_access_control(&config.access);
    
    let filesystem_handler = FilesystemHandler::new(access_control, &config.performance);
    let stats = filesystem_handler.get_statistics().await;
    
    assert_eq!(stats.total_operations, 0);
    assert_eq!(stats.error_count, 0);
    assert_eq!(stats.bytes_read, 0);
    assert_eq!(stats.bytes_written, 0);
}

#[tokio::test]
async fn test_read_file_success() {
    setup_test_logging();
    let temp_dir = create_temp_dir();
    create_test_directory_structure(temp_dir.path());
    let config = create_test_config(temp_dir.path());
    let access_control = create_test_access_control(&config.access);
    
    let filesystem_handler = FilesystemHandler::new(access_control, &config.performance);
    let request_id = Uuid::new_v4();
    let file_path = temp_dir.path().join("allowed/test.txt").to_string_lossy().to_string();
    
    let result = filesystem_handler.handle_read_file(request_id, file_path, None, None).await;
    
    assert!(result.is_some(), "Should return a response");
    let response = result.unwrap();
    
    // Check if it's a ReadFileResponse (would need to match on the actual message type)
    // For now, we just verify we got a response
    
    let stats = filesystem_handler.get_statistics().await;
    assert_eq!(stats.total_operations, 1);
    assert!(stats.bytes_read > 0);
}

#[tokio::test]
async fn test_read_file_with_offset_and_length() {
    setup_test_logging();
    let temp_dir = create_temp_dir();
    create_test_directory_structure(temp_dir.path());
    let config = create_test_config(temp_dir.path());
    let access_control = create_test_access_control(&config.access);
    
    // Create a larger test file
    let large_file_path = temp_dir.path().join("allowed/large.txt");
    create_test_file(&large_file_path, "0123456789abcdefghijklmnopqrstuvwxyz");
    
    let filesystem_handler = FilesystemHandler::new(access_control, &config.performance);
    let request_id = Uuid::new_v4();
    let file_path = large_file_path.to_string_lossy().to_string();
    
    // Read 5 bytes starting from offset 10
    let result = filesystem_handler.handle_read_file(request_id, file_path, Some(10), Some(5)).await;
    
    assert!(result.is_some(), "Should return a response");
    
    let stats = filesystem_handler.get_statistics().await;
    assert_eq!(stats.total_operations, 1);
    assert!(stats.bytes_read > 0);
}

#[tokio::test]
async fn test_read_file_access_denied() {
    setup_test_logging();
    let temp_dir = create_temp_dir();
    create_test_directory_structure(temp_dir.path());
    let config = create_test_config(temp_dir.path());
    let access_control = create_test_access_control(&config.access);
    
    let filesystem_handler = FilesystemHandler::new(access_control, &config.performance);
    let request_id = Uuid::new_v4();
    let denied_file_path = temp_dir.path().join("denied/secret.txt").to_string_lossy().to_string();
    
    let result = filesystem_handler.handle_read_file(request_id, denied_file_path, None, None).await;
    
    assert!(result.is_some(), "Should return a response");
    // The response should contain an error
    
    let stats = filesystem_handler.get_statistics().await;
    assert_eq!(stats.total_operations, 1);
    assert_eq!(stats.error_count, 1);
}

#[tokio::test]
async fn test_read_nonexistent_file() {
    setup_test_logging();
    let temp_dir = create_temp_dir();
    create_test_directory_structure(temp_dir.path());
    let config = create_test_config(temp_dir.path());
    let access_control = create_test_access_control(&config.access);
    
    let filesystem_handler = FilesystemHandler::new(access_control, &config.performance);
    let request_id = Uuid::new_v4();
    let nonexistent_path = temp_dir.path().join("allowed/nonexistent.txt").to_string_lossy().to_string();
    
    let result = filesystem_handler.handle_read_file(request_id, nonexistent_path, None, None).await;
    
    assert!(result.is_some(), "Should return a response");
    // The response should contain an error
    
    let stats = filesystem_handler.get_statistics().await;
    assert_eq!(stats.total_operations, 1);
    assert_eq!(stats.error_count, 1);
}

#[tokio::test]
async fn test_write_file_create_new() {
    setup_test_logging();
    let temp_dir = create_temp_dir();
    create_test_directory_structure(temp_dir.path());
    let config = create_test_config(temp_dir.path());
    let access_control = create_test_access_control(&config.access);
    
    let filesystem_handler = FilesystemHandler::new(access_control, &config.performance);
    let request_id = Uuid::new_v4();
    let new_file_path = temp_dir.path().join("allowed/new_file.txt").to_string_lossy().to_string();
    let data = b"Hello, new file!".to_vec();
    
    let result = filesystem_handler.handle_write_file(request_id, new_file_path.clone(), data.clone(), None, true).await;
    
    assert!(result.is_some(), "Should return a response");
    
    // Verify file was created
    assert_path_exists(&new_file_path);
    assert_file_content(&new_file_path, "Hello, new file!");
    
    let stats = filesystem_handler.get_statistics().await;
    assert_eq!(stats.total_operations, 1);
    assert_eq!(stats.bytes_written, data.len() as u64);
}

#[tokio::test]
async fn test_write_file_with_offset() {
    setup_test_logging();
    let temp_dir = create_temp_dir();
    create_test_directory_structure(temp_dir.path());
    let config = create_test_config(temp_dir.path());
    let access_control = create_test_access_control(&config.access);
    
    // Create initial file
    let file_path = temp_dir.path().join("allowed/offset_test.txt");
    create_test_file(&file_path, "0123456789");
    
    let filesystem_handler = FilesystemHandler::new(access_control, &config.performance);
    let request_id = Uuid::new_v4();
    let file_path_str = file_path.to_string_lossy().to_string();
    let data = b"XXX".to_vec();
    
    let result = filesystem_handler.handle_write_file(request_id, file_path_str, data, Some(3), false).await;
    
    assert!(result.is_some(), "Should return a response");
    
    // Verify file content was modified at offset
    assert_file_content(&file_path, "012XXX6789");
    
    let stats = filesystem_handler.get_statistics().await;
    assert_eq!(stats.total_operations, 1);
    assert_eq!(stats.bytes_written, 3);
}

#[tokio::test]
async fn test_write_file_readonly_path() {
    setup_test_logging();
    let temp_dir = create_temp_dir();
    create_test_directory_structure(temp_dir.path());
    let config = create_test_config(temp_dir.path());
    let access_control = create_test_access_control(&config.access);
    
    let filesystem_handler = FilesystemHandler::new(access_control, &config.performance);
    let request_id = Uuid::new_v4();
    let readonly_file_path = temp_dir.path().join("readonly/readonly.txt").to_string_lossy().to_string();
    let data = b"should not be written".to_vec();
    
    let result = filesystem_handler.handle_write_file(request_id, readonly_file_path, data, None, false).await;
    
    assert!(result.is_some(), "Should return a response");
    // The response should contain an error
    
    let stats = filesystem_handler.get_statistics().await;
    assert_eq!(stats.total_operations, 1);
    assert_eq!(stats.error_count, 1);
    assert_eq!(stats.bytes_written, 0);
}

#[tokio::test]
async fn test_list_directory_success() {
    setup_test_logging();
    let temp_dir = create_temp_dir();
    create_test_directory_structure(temp_dir.path());
    let config = create_test_config(temp_dir.path());
    let access_control = create_test_access_control(&config.access);
    
    let filesystem_handler = FilesystemHandler::new(access_control, &config.performance);
    let request_id = Uuid::new_v4();
    let dir_path = temp_dir.path().join("allowed").to_string_lossy().to_string();
    
    let result = filesystem_handler.handle_list_directory(request_id, dir_path).await;
    
    assert!(result.is_some(), "Should return a response");
    
    let stats = filesystem_handler.get_statistics().await;
    assert_eq!(stats.total_operations, 1);
}

#[tokio::test]
async fn test_list_directory_access_denied() {
    setup_test_logging();
    let temp_dir = create_temp_dir();
    create_test_directory_structure(temp_dir.path());
    let config = create_test_config(temp_dir.path());
    let access_control = create_test_access_control(&config.access);
    
    let filesystem_handler = FilesystemHandler::new(access_control, &config.performance);
    let request_id = Uuid::new_v4();
    let denied_dir_path = temp_dir.path().join("denied").to_string_lossy().to_string();
    
    let result = filesystem_handler.handle_list_directory(request_id, denied_dir_path).await;
    
    assert!(result.is_some(), "Should return a response");
    // The response should contain an error
    
    let stats = filesystem_handler.get_statistics().await;
    assert_eq!(stats.total_operations, 1);
    assert_eq!(stats.error_count, 1);
}

#[tokio::test]
async fn test_list_nonexistent_directory() {
    setup_test_logging();
    let temp_dir = create_temp_dir();
    create_test_directory_structure(temp_dir.path());
    let config = create_test_config(temp_dir.path());
    let access_control = create_test_access_control(&config.access);
    
    let filesystem_handler = FilesystemHandler::new(access_control, &config.performance);
    let request_id = Uuid::new_v4();
    let nonexistent_dir = temp_dir.path().join("allowed/nonexistent").to_string_lossy().to_string();
    
    let result = filesystem_handler.handle_list_directory(request_id, nonexistent_dir).await;
    
    assert!(result.is_some(), "Should return a response");
    // The response should contain an error
    
    let stats = filesystem_handler.get_statistics().await;
    assert_eq!(stats.total_operations, 1);
    assert_eq!(stats.error_count, 1);
}

#[tokio::test]
async fn test_get_metadata_success() {
    setup_test_logging();
    let temp_dir = create_temp_dir();
    create_test_directory_structure(temp_dir.path());
    let config = create_test_config(temp_dir.path());
    let access_control = create_test_access_control(&config.access);
    
    let filesystem_handler = FilesystemHandler::new(access_control, &config.performance);
    let request_id = Uuid::new_v4();
    let file_path = temp_dir.path().join("allowed/test.txt").to_string_lossy().to_string();
    
    let result = filesystem_handler.handle_get_metadata(request_id, file_path, true).await;
    
    assert!(result.is_some(), "Should return a response");
    
    let stats = filesystem_handler.get_statistics().await;
    assert_eq!(stats.total_operations, 1);
}

#[tokio::test]
async fn test_get_metadata_directory() {
    setup_test_logging();
    let temp_dir = create_temp_dir();
    create_test_directory_structure(temp_dir.path());
    let config = create_test_config(temp_dir.path());
    let access_control = create_test_access_control(&config.access);
    
    let filesystem_handler = FilesystemHandler::new(access_control, &config.performance);
    let request_id = Uuid::new_v4();
    let dir_path = temp_dir.path().join("allowed").to_string_lossy().to_string();
    
    let result = filesystem_handler.handle_get_metadata(request_id, dir_path, true).await;
    
    assert!(result.is_some(), "Should return a response");
    
    let stats = filesystem_handler.get_statistics().await;
    assert_eq!(stats.total_operations, 1);
}

#[tokio::test]
async fn test_create_directory_success() {
    setup_test_logging();
    let temp_dir = create_temp_dir();
    create_test_directory_structure(temp_dir.path());
    let config = create_test_config(temp_dir.path());
    let access_control = create_test_access_control(&config.access);
    
    let filesystem_handler = FilesystemHandler::new(access_control, &config.performance);
    let request_id = Uuid::new_v4();
    let new_dir_path = temp_dir.path().join("allowed/new_directory").to_string_lossy().to_string();
    
    let result = filesystem_handler.handle_create_directory(request_id, new_dir_path.clone(), 0o755).await;
    
    assert!(result.is_some(), "Should return a response");
    
    // Verify directory was created
    assert_path_exists(&new_dir_path);
    assert!(std::path::Path::new(&new_dir_path).is_dir());
    
    let stats = filesystem_handler.get_statistics().await;
    assert_eq!(stats.total_operations, 1);
}

#[tokio::test]
async fn test_create_directory_recursive() {
    setup_test_logging();
    let temp_dir = create_temp_dir();
    create_test_directory_structure(temp_dir.path());
    let config = create_test_config(temp_dir.path());
    let access_control = create_test_access_control(&config.access);
    
    let filesystem_handler = FilesystemHandler::new(access_control, &config.performance);
    let request_id = Uuid::new_v4();
    let nested_dir_path = temp_dir.path().join("allowed/deep/nested/directory").to_string_lossy().to_string();
    
    let result = filesystem_handler.handle_create_directory(request_id, nested_dir_path.clone(), 0o755).await;
    
    assert!(result.is_some(), "Should return a response");
    
    // Verify nested directories were created
    assert_path_exists(&nested_dir_path);
    assert!(std::path::Path::new(&nested_dir_path).is_dir());
    
    let stats = filesystem_handler.get_statistics().await;
    assert_eq!(stats.total_operations, 1);
}

#[tokio::test]
async fn test_delete_file_success() {
    setup_test_logging();
    let temp_dir = create_temp_dir();
    create_test_directory_structure(temp_dir.path());
    let config = create_test_config(temp_dir.path());
    let access_control = create_test_access_control(&config.access);
    
    // Create a file to delete
    let file_to_delete = temp_dir.path().join("allowed/delete_me.txt");
    create_test_file(&file_to_delete, "delete me");
    assert_path_exists(&file_to_delete);
    
    let filesystem_handler = FilesystemHandler::new(access_control, &config.performance);
    let request_id = Uuid::new_v4();
    let file_path = file_to_delete.to_string_lossy().to_string();
    
    let result = filesystem_handler.handle_delete_file(request_id, file_path).await;
    
    assert!(result.is_some(), "Should return a response");
    
    // Verify file was deleted
    assert_path_not_exists(&file_to_delete);
    
    let stats = filesystem_handler.get_statistics().await;
    assert_eq!(stats.total_operations, 1);
}

#[tokio::test]
async fn test_delete_file_readonly_path() {
    setup_test_logging();
    let temp_dir = create_temp_dir();
    create_test_directory_structure(temp_dir.path());
    let config = create_test_config(temp_dir.path());
    let access_control = create_test_access_control(&config.access);
    
    let filesystem_handler = FilesystemHandler::new(access_control, &config.performance);
    let request_id = Uuid::new_v4();
    let readonly_file = temp_dir.path().join("readonly/readonly.txt").to_string_lossy().to_string();
    
    let result = filesystem_handler.handle_delete_file(request_id, readonly_file.clone()).await;
    
    assert!(result.is_some(), "Should return a response");
    // The response should contain an error
    
    // Verify file still exists
    assert_path_exists(&readonly_file);
    
    let stats = filesystem_handler.get_statistics().await;
    assert_eq!(stats.total_operations, 1);
    assert_eq!(stats.error_count, 1);
}

#[tokio::test]
async fn test_concurrent_filesystem_operations() {
    setup_test_logging();
    let temp_dir = create_temp_dir();
    create_test_directory_structure(temp_dir.path());
    let config = create_test_config(temp_dir.path());
    let access_control = create_test_access_control(&config.access);
    
    let filesystem_handler = Arc::new(FilesystemHandler::new(access_control, &config.performance));
    let file_path = temp_dir.path().join("allowed/test.txt").to_string_lossy().to_string();
    
    // Run multiple concurrent read operations
    let tasks: Vec<_> = (0..5).map(|i| {
        let handler = filesystem_handler.clone();
        let path = file_path.clone();
        tokio::spawn(async move {
            let request_id = Uuid::new_v4();
            handler.handle_read_file(request_id, path, None, None).await
        })
    }).collect();
    
    let results: Vec<_> = futures::future::join_all(tasks).await;
    
    // All should return responses
    for result in results {
        assert!(result.unwrap().is_some(), "All concurrent operations should return responses");
    }
    
    // Verify statistics
    let stats = filesystem_handler.get_statistics().await;
    assert_eq!(stats.total_operations, 5);
}

#[tokio::test]
async fn test_filesystem_performance_stats() {
    setup_test_logging();
    let temp_dir = create_temp_dir();
    create_test_directory_structure(temp_dir.path());
    let config = create_test_config(temp_dir.path());
    let access_control = create_test_access_control(&config.access);
    
    let filesystem_handler = FilesystemHandler::new(access_control, &config.performance);
    let file_path = temp_dir.path().join("allowed/test.txt").to_string_lossy().to_string();
    
    // Perform several operations
    for i in 0..3 {
        let request_id = Uuid::new_v4();
        filesystem_handler.handle_read_file(request_id, file_path.clone(), None, None).await;
    }
    
    let perf_stats = filesystem_handler.get_performance_stats().await;
    assert!(perf_stats.avg_response_time_ms >= 0.0);
    assert!(perf_stats.operations_per_second >= 0.0);
    assert!(perf_stats.bytes_read > 0);
    
    let stats = filesystem_handler.get_statistics().await;
    assert_eq!(stats.total_operations, 3);
}

#[tokio::test]
async fn test_filesystem_cleanup_operations() {
    setup_test_logging();
    let temp_dir = create_temp_dir();
    create_test_directory_structure(temp_dir.path());
    let config = create_test_config(temp_dir.path());
    let access_control = create_test_access_control(&config.access);
    
    let filesystem_handler = FilesystemHandler::new(access_control, &config.performance);
    
    // Test cleanup operations (even though they're placeholders for now)
    let temp_files_cleaned = filesystem_handler.cleanup_temp_files().await;
    assert_eq!(temp_files_cleaned, 0); // Currently returns 0 as placeholder
    
    filesystem_handler.cleanup_old_metrics().await;
    // Should complete without error
}
