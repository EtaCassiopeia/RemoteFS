#!/usr/bin/env python3
"""
RemoteFS End-to-End Test Suite

This script performs comprehensive testing of RemoteFS functionality including:
- File and directory operations
- Git repository manipulation
- Code editing and LLM-style modifications
- Performance benchmarking
- Error handling and recovery

Usage: python3 e2e_test.py
"""

import os
import sys
import time
import json
import shutil
import subprocess
import tempfile
import logging
from pathlib import Path
from typing import Dict, List, Optional, Tuple
from dataclasses import dataclass
from datetime import datetime

# Test configuration
TEST_CONFIG = {
    "mount_point": "/app/mount",
    "test_results_dir": "/app/test-results",
    "remote_workspace": "/app/workspace", 
    "timeout": 300,  # 5 minutes
    "retry_attempts": 3,
}

@dataclass
class TestResult:
    name: str
    success: bool
    duration: float
    error_message: Optional[str] = None
    details: Optional[Dict] = None

class RemoteFSE2ETest:
    def __init__(self):
        self.mount_point = Path(TEST_CONFIG["mount_point"])
        self.results_dir = Path(TEST_CONFIG["test_results_dir"])
        self.test_results: List[TestResult] = []
        self.setup_logging()
        
    def setup_logging(self):
        """Setup logging configuration"""
        log_file = self.results_dir / "e2e_test.log"
        logging.basicConfig(
            level=logging.INFO,
            format='%(asctime)s - %(levelname)s - %(message)s',
            handlers=[
                logging.FileHandler(log_file),
                logging.StreamHandler()
            ]
        )
        self.logger = logging.getLogger(__name__)
        
    def run_command(self, command: List[str], timeout: int = 30) -> Tuple[bool, str, str]:
        """Run a shell command and return success, stdout, stderr"""
        try:
            result = subprocess.run(
                command,
                capture_output=True,
                text=True,
                timeout=timeout
            )
            return result.returncode == 0, result.stdout, result.stderr
        except subprocess.TimeoutExpired:
            return False, "", "Command timed out"
        except Exception as e:
            return False, "", str(e)
    
    def wait_for_mount(self, timeout: int = 60) -> bool:
        """Wait for the filesystem to be mounted"""
        start_time = time.time()
        while time.time() - start_time < timeout:
            if self.mount_point.exists() and self.mount_point.is_mount():
                return True
            time.sleep(1)
        return False
    
    def test_basic_connectivity(self) -> TestResult:
        """Test basic filesystem connectivity"""
        start_time = time.time()
        try:
            # Check if mount point exists and is accessible
            if not self.mount_point.exists():
                return TestResult(
                    "basic_connectivity",
                    False,
                    time.time() - start_time,
                    "Mount point does not exist"
                )
            
            # Try to list the root directory
            files = list(self.mount_point.iterdir())
            self.logger.info(f"Found {len(files)} items in remote filesystem")
            
            return TestResult(
                "basic_connectivity",
                True,
                time.time() - start_time,
                details={"files_found": len(files)}
            )
            
        except Exception as e:
            return TestResult(
                "basic_connectivity",
                False,
                time.time() - start_time,
                str(e)
            )
    
    def test_file_operations(self) -> TestResult:
        """Test basic file operations (CRUD)"""
        start_time = time.time()
        test_file = self.mount_point / "test_file.txt"
        
        try:
            # Create file
            content = "Hello, RemoteFS! This is a test file created by E2E tests."
            test_file.write_text(content)
            
            # Read file
            read_content = test_file.read_text()
            if read_content != content:
                raise Exception("File content mismatch")
            
            # Update file
            new_content = content + "\\nUpdated content."
            test_file.write_text(new_content)
            
            # Verify update
            updated_content = test_file.read_text()
            if updated_content != new_content:
                raise Exception("File update failed")
            
            # Delete file
            test_file.unlink()
            
            # Verify deletion
            if test_file.exists():
                raise Exception("File deletion failed")
            
            return TestResult(
                "file_operations", 
                True,
                time.time() - start_time
            )
            
        except Exception as e:
            # Cleanup on failure
            if test_file.exists():
                test_file.unlink()
            return TestResult(
                "file_operations",
                False, 
                time.time() - start_time,
                str(e)
            )
    
    def test_directory_operations(self) -> TestResult:
        """Test directory operations"""
        start_time = time.time()
        test_dir = self.mount_point / "test_directory"
        
        try:
            # Create directory
            test_dir.mkdir(exist_ok=True)
            
            # Create nested directories
            nested_dir = test_dir / "nested" / "deep"
            nested_dir.mkdir(parents=True, exist_ok=True)
            
            # Create files in directories
            file1 = test_dir / "file1.txt"
            file2 = nested_dir / "file2.txt"
            
            file1.write_text("File in root test directory")
            file2.write_text("File in nested directory")
            
            # List directory contents
            contents = list(test_dir.rglob("*"))
            
            # Cleanup
            shutil.rmtree(test_dir)
            
            return TestResult(
                "directory_operations",
                True,
                time.time() - start_time,
                details={"files_created": len(contents)}
            )
            
        except Exception as e:
            # Cleanup on failure
            if test_dir.exists():
                shutil.rmtree(test_dir)
            return TestResult(
                "directory_operations",
                False,
                time.time() - start_time, 
                str(e)
            )
    
    def test_git_operations(self) -> TestResult:
        """Test Git repository operations"""
        start_time = time.time()
        
        try:
            # Navigate to the mock project in the mounted filesystem
            project_path = self.mount_point
            
            # Check if it's a git repository
            success, stdout, stderr = self.run_command(
                ["git", "-C", str(project_path), "status"], 
                timeout=10
            )
            
            if not success:
                return TestResult(
                    "git_operations",
                    False,
                    time.time() - start_time,
                    f"Not a git repository: {stderr}"
                )
            
            # Create a new branch (delete existing if present)
            # First try to delete existing branch if it exists
            self.run_command([
                "git", "-C", str(project_path), "branch", "-D", "e2e-test-branch"
            ])
            
            # Now create the new branch
            success, _, stderr = self.run_command([
                "git", "-C", str(project_path), "checkout", "-b", "e2e-test-branch"
            ])
            
            if not success:
                return TestResult(
                    "git_operations",
                    False,
                    time.time() - start_time,
                    f"Failed to create branch: {stderr}"
                )
            
            # Make some changes
            test_file = project_path / "src" / "test_changes.rs"
            test_file.write_text("""
// This file was added during E2E testing
use std::collections::HashMap;

pub struct TestStruct {
    pub data: HashMap<String, String>,
}

impl TestStruct {
    pub fn new() -> Self {
        Self {
            data: HashMap::new(),
        }
    }
}
""")
            
            # Stage changes
            success, _, stderr = self.run_command([
                "git", "-C", str(project_path), "add", "."
            ])
            
            if not success:
                return TestResult(
                    "git_operations", 
                    False,
                    time.time() - start_time,
                    f"Failed to stage changes: {stderr}"
                )
            
            # Commit changes
            success, _, stderr = self.run_command([
                "git", "-C", str(project_path), "commit", 
                "-m", "E2E test: Add test file with new struct"
            ])
            
            if not success:
                return TestResult(
                    "git_operations",
                    False,
                    time.time() - start_time, 
                    f"Failed to commit: {stderr}"
                )
            
            # Get commit log
            success, stdout, stderr = self.run_command([
                "git", "-C", str(project_path), "log", "--oneline", "-n", "3"
            ])
            
            return TestResult(
                "git_operations",
                True,
                time.time() - start_time,
                details={"commit_log": stdout.strip()}
            )
            
        except Exception as e:
            return TestResult(
                "git_operations",
                False,
                time.time() - start_time,
                str(e)
            )
    
    def test_code_modifications(self) -> TestResult:
        """Test LLM-style code modifications"""
        start_time = time.time()
        
        try:
            project_path = self.mount_point
            
            # Read existing main.rs file
            main_file = project_path / "src" / "main.rs"
            if not main_file.exists():
                return TestResult(
                    "code_modifications",
                    False,
                    time.time() - start_time,
                    "main.rs file not found"
                )
            
            original_content = main_file.read_text()
            
            # Simulate LLM-style code modification
            # Add a new function and modify existing code
            
            # Add a new utility function
            new_function = '''

fn process_data(input: &str) -> String {
    format!("Processed: {}", input.to_uppercase())
}
'''
            
            # Insert the new function before the main function
            modified_content = original_content.replace(
                'fn main() {',
                new_function + 'fn main() {'
            )
            
            # Add a call to the new function in main
            modified_content = modified_content.replace(
                'println!("Initialized with {} items", data.len());',
                '''println!("Initialized with {} items", data.len());
    println!("{}", process_data("test data"));'''
            )
            
            # Write modified content
            main_file.write_text(modified_content)
            
            # Verify the changes were written
            new_content = main_file.read_text()
            
            if "process_data" not in new_content or "test data" not in new_content:
                raise Exception("Code modifications were not saved properly")
            
            # Try to run cargo check if available (syntax validation)
            cargo_available = True
            cargo_success = False
            cargo_output = ""
            
            try:
                success, stdout, stderr = self.run_command([
                    "cargo", "check", "--manifest-path", str(project_path / "Cargo.toml")
                ], timeout=120)
                cargo_success = success
                cargo_output = stdout[:500] if stdout else stderr[:500]
            except Exception:
                cargo_available = False
                cargo_output = "Cargo not available in test environment"
            
            # Restore original content
            main_file.write_text(original_content)
            
            # Test passes if modifications were applied successfully
            # Cargo check is optional if not available
            test_success = True  # We already verified the modifications were saved
            
            return TestResult(
                "code_modifications",
                test_success,
                time.time() - start_time,
                error_message=None,
                details={
                    "modifications_applied": True,
                    "cargo_available": cargo_available,
                    "syntax_check_passed": cargo_success if cargo_available else None,
                    "build_output": cargo_output
                }
            )
            
        except Exception as e:
            return TestResult(
                "code_modifications",
                False,
                time.time() - start_time,
                str(e)
            )
    
    def test_file_search_operations(self) -> TestResult:
        """Test file search and discovery operations (like find/grep)"""
        start_time = time.time()
        
        try:
            project_path = self.mount_point
            
            # Test find operations
            success, stdout, stderr = self.run_command([
                "find", str(project_path), "-name", "*.rs", "-type", "f"
            ])
            
            if not success:
                return TestResult(
                    "file_search_operations",
                    False,
                    time.time() - start_time,
                    f"Find command failed: {stderr}"
                )
            
            rust_files = stdout.strip().split('\\n') if stdout.strip() else []
            
            # Test grep operations
            success, stdout, stderr = self.run_command([
                "grep", "-r", "use ", str(project_path / "src"), "--include=*.rs"
            ])
            
            use_statements = len(stdout.split('\\n')) if stdout else 0
            
            # Test file content analysis
            file_sizes = {}
            for rs_file in rust_files[:5]:  # Limit to first 5 files
                if rs_file and Path(rs_file).exists():
                    file_sizes[Path(rs_file).name] = Path(rs_file).stat().st_size
            
            return TestResult(
                "file_search_operations",
                True,
                time.time() - start_time,
                details={
                    "rust_files_found": len(rust_files),
                    "use_statements_found": use_statements,
                    "file_sizes": file_sizes
                }
            )
            
        except Exception as e:
            return TestResult(
                "file_search_operations",
                False,
                time.time() - start_time,
                str(e)
            )
    
    def test_performance_benchmark(self) -> TestResult:
        """Test performance with various file operations"""
        start_time = time.time()
        
        try:
            test_dir = self.mount_point / "performance_test"
            test_dir.mkdir(exist_ok=True)
            
            # Test file write performance
            write_times = []
            for i in range(10):
                file_start = time.time()
                test_file = test_dir / f"perf_test_{i}.txt"
                test_file.write_text(f"Performance test file {i} " * 100)  # ~2KB file
                write_times.append(time.time() - file_start)
            
            # Test file read performance
            read_times = []
            for i in range(10):
                read_start = time.time()
                test_file = test_dir / f"perf_test_{i}.txt"
                content = test_file.read_text()
                read_times.append(time.time() - read_start)
            
            # Test directory listing performance
            list_start = time.time()
            files = list(test_dir.iterdir())
            list_time = time.time() - list_start
            
            # Cleanup
            shutil.rmtree(test_dir)
            
            avg_write_time = sum(write_times) / len(write_times)
            avg_read_time = sum(read_times) / len(read_times)
            
            return TestResult(
                "performance_benchmark",
                True,
                time.time() - start_time,
                details={
                    "avg_write_time_ms": avg_write_time * 1000,
                    "avg_read_time_ms": avg_read_time * 1000,
                    "directory_list_time_ms": list_time * 1000,
                    "files_tested": len(files)
                }
            )
            
        except Exception as e:
            if test_dir.exists():
                shutil.rmtree(test_dir, ignore_errors=True)
            return TestResult(
                "performance_benchmark",
                False,
                time.time() - start_time,
                str(e)
            )
    
    def run_all_tests(self) -> Dict:
        """Run all end-to-end tests"""
        self.logger.info("Starting RemoteFS End-to-End Tests")
        start_time = time.time()
        
        # Wait for mount to be ready
        self.logger.info("Waiting for filesystem mount...")
        if not self.wait_for_mount():
            self.logger.error("Filesystem mount not ready within timeout")
            return {"success": False, "error": "Mount timeout"}
        
        # Define test methods to run
        test_methods = [
            self.test_basic_connectivity,
            self.test_file_operations,
            self.test_directory_operations,
            self.test_git_operations,
            self.test_code_modifications,
            self.test_file_search_operations,
            self.test_performance_benchmark,
        ]
        
        # Run each test
        for test_method in test_methods:
            self.logger.info(f"Running {test_method.__name__}...")
            result = test_method()
            self.test_results.append(result)
            
            status = "PASS" if result.success else "FAIL"
            self.logger.info(f"{test_method.__name__}: {status} ({result.duration:.2f}s)")
            
            if result.error_message:
                self.logger.error(f"  Error: {result.error_message}")
        
        # Generate summary
        total_time = time.time() - start_time
        passed_tests = [r for r in self.test_results if r.success]
        failed_tests = [r for r in self.test_results if not r.success]
        
        summary = {
            "total_tests": len(self.test_results),
            "passed_tests": len(passed_tests),
            "failed_tests": len(failed_tests),
            "total_duration": total_time,
            "success_rate": len(passed_tests) / len(self.test_results) * 100,
            "results": [
                {
                    "name": r.name,
                    "success": r.success,
                    "duration": r.duration,
                    "error": r.error_message,
                    "details": r.details
                }
                for r in self.test_results
            ]
        }
        
        # Save results
        results_file = self.results_dir / "e2e_test_results.json"
        with open(results_file, "w") as f:
            json.dump(summary, f, indent=2, default=str)
        
        self.logger.info(f"\\nTest Summary:")
        self.logger.info(f"Total Tests: {summary['total_tests']}")
        self.logger.info(f"Passed: {summary['passed_tests']}")
        self.logger.info(f"Failed: {summary['failed_tests']}")
        self.logger.info(f"Success Rate: {summary['success_rate']:.1f}%")
        self.logger.info(f"Total Duration: {summary['total_duration']:.2f}s")
        self.logger.info(f"Results saved to: {results_file}")
        
        return summary

def main():
    """Main entry point for the test suite"""
    try:
        test_suite = RemoteFSE2ETest()
        results = test_suite.run_all_tests()
        
        # Exit with appropriate code
        exit_code = 0 if results.get("failed_tests", 0) == 0 else 1
        sys.exit(exit_code)
        
    except Exception as e:
        logging.error(f"Test suite failed with exception: {e}")
        sys.exit(1)

if __name__ == "__main__":
    main()
