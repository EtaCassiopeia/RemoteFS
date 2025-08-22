# RemoteFS End-to-End Test Suite

This directory contains a comprehensive end-to-end test setup for RemoteFS that simulates a real-world distributed development environment. The test setup creates a network of Docker containers that include a relay server, remote agent (simulating a macOS development machine), and test client, with a realistic source code project for testing file operations.

## Overview

The E2E test environment simulates:

- **Remote macOS Machine**: A Docker container with development tools and a Git repository
- **Relay Server**: Central routing and authentication service
- **Test Client**: Container that mounts the remote filesystem and runs tests
- **NFS Server**: Alternative NFS-based mounting for comparison

## Test Coverage

The comprehensive test suite covers:

✅ **Basic Operations**
- File create, read, update, delete (CRUD)
- Directory creation and traversal
- File system connectivity

✅ **Advanced Operations** 
- Git repository manipulation (branch, commit, status)
- Code modifications (LLM-style editing)
- File search operations (find, grep)
- Performance benchmarking

✅ **Real-World Scenarios**
- Simulates AI/LLM development workflows
- Tests operations that Warp Terminal would perform
- Validates concurrent file operations
- Error handling and recovery

## Architecture

```
┌─────────────────┐    ┌─────────────────┐    ┌─────────────────┐
│   Test Client   │────│  Relay Server   │────│ Remote Agent    │
│                 │    │                 │    │ (macOS sim)     │
│ - Runs tests    │    │ - Routes msgs   │    │ - Git repo      │
│ - Mounts FS     │    │ - Authentication│    │ - Source code   │
│ - Benchmarks    │    │ - Load balancing│    │ - Dev tools     │
└─────────────────┘    └─────────────────┘    └─────────────────┘
         │                       │                       │
         └───────── NFS Server ──┘───────────────────────┘
                    (Alternative mounting)
```

## Prerequisites

- **Docker** (>= 20.10)
- **Docker Compose** (>= 2.0)
- **Sufficient disk space** (~2GB for images and test data)
- **Available ports**: 8080, 8081, 2049

### Optional
- **sudo access** (for NFS mount testing)
- **jq** (for enhanced test reporting)

## Quick Start

### 1. Setup the Environment

```bash
cd e2e-tests
./scripts/setup.sh
```

This will:
- Build all Docker images
- Start all services (relay, agent, NFS server, test client)
- Wait for services to become healthy
- Display service status and endpoints

### 2. Run the Tests

```bash
./scripts/run_tests.sh
```

This will:
- Mount the remote filesystem
- Execute comprehensive E2E tests
- Generate detailed test reports
- Show performance metrics

### 3. Cleanup (Optional)

```bash
./scripts/teardown.sh
```

## Detailed Usage

### Setup Options

```bash
# Basic setup
./scripts/setup.sh

# Skip rebuilding images (faster restarts)
./scripts/setup.sh --no-build

# Force recreate all containers
./scripts/setup.sh --force-recreate

# Show help
./scripts/setup.sh help
```

### Test Execution Options

```bash
# Run all tests
./scripts/run_tests.sh

# Include NFS mount testing (requires sudo)
./scripts/run_tests.sh --with-nfs

# Show detailed debug information
./scripts/run_tests.sh --debug

# Show help
./scripts/run_tests.sh --help
```

### Teardown Options

```bash
# Basic cleanup (stop containers, unmount filesystems)
./scripts/teardown.sh

# Complete cleanup (also remove volumes and images)
./scripts/teardown.sh --full

# Preserve test results during cleanup
./scripts/teardown.sh --preserve-results

# Show help
./scripts/teardown.sh --help
```

## Manual Testing

You can also manually interact with the environment:

### Access the Remote Filesystem

```bash
# The remote filesystem is mounted at:
ls /tmp/remotefs-mount

# Or inside the test client container:
docker-compose exec test-client ls /app/mount
```

### Execute Commands on Remote Agent

```bash
# Run commands on the simulated macOS machine
docker-compose exec remote-agent bash

# Example: Check git status
docker-compose exec remote-agent git -C /app/workspace status

# Example: List files
docker-compose exec remote-agent find /app/workspace -name "*.rs"
```

### Monitor Services

```bash
# Check service status
docker-compose ps

# View logs
docker-compose logs -f relay
docker-compose logs -f remote-agent
docker-compose logs -f test-client

# View real-time logs from all services
docker-compose logs -f
```

## Test Results

Test results are automatically saved to:
- `./test-results/e2e_test_results.json` - Detailed JSON results
- `./test-results/e2e_test.log` - Test execution logs  
- `./test-results/agent.log` - Remote agent logs
- `./test-results/client.log` - Test client logs

### Sample Test Results

```json
{
  "total_tests": 7,
  "passed_tests": 7,
  "failed_tests": 0,
  "total_duration": 45.2,
  "success_rate": 100.0,
  "results": [
    {
      "name": "basic_connectivity",
      "success": true,
      "duration": 1.2,
      "details": {"files_found": 8}
    },
    {
      "name": "performance_benchmark", 
      "success": true,
      "duration": 8.1,
      "details": {
        "avg_write_time_ms": 12.3,
        "avg_read_time_ms": 8.7,
        "directory_list_time_ms": 2.1
      }
    }
  ]
}
```

## Test Scenarios

### File Operations Test
- Creates, reads, updates, and deletes files
- Tests content integrity
- Validates error handling

### Directory Operations Test  
- Creates nested directory structures
- Tests recursive operations
- Validates cleanup

### Git Operations Test
- Creates branches
- Makes commits
- Tests repository state consistency
- Validates git command execution over RemoteFS

### Code Modifications Test
- Simulates LLM-style code editing
- Modifies Rust source files
- Tests syntax validation
- Verifies changes persist correctly

### File Search Operations Test
- Tests `find` command functionality
- Tests `grep` operations across files
- Validates search performance

### Performance Benchmark Test
- Measures file I/O performance
- Tests concurrent operations
- Provides performance metrics

## Mock Project

The test environment includes a realistic mock project:

```
mock-project/
├── Cargo.toml              # Rust project configuration
├── README.md               # Project documentation
├── .env                    # Environment configuration
├── .git/                   # Git repository
└── src/
    ├── main.rs             # Main application
    ├── models.rs           # Data models  
    ├── handlers.rs         # HTTP handlers
    └── services.rs         # Business logic
```

This project simulates a typical web service that an AI/LLM might work with, including:
- REST API endpoints
- Database models
- Error handling
- Configuration management

## Troubleshooting

### Services Won't Start

```bash
# Check Docker status
docker --version
docker-compose --version

# View detailed logs
./scripts/setup.sh
docker-compose logs

# Force rebuild and restart
./scripts/teardown.sh --full
./scripts/setup.sh
```

### Mount Issues

```bash
# Check if filesystem is mounted
mount | grep remotefs

# Manually unmount
fusermount -u /tmp/remotefs-mount
# or
umount /tmp/remotefs-mount

# Check FUSE availability
ls -la /dev/fuse
```

### Test Failures

```bash
# Run with debug information
./scripts/run_tests.sh --debug

# Check individual service logs
docker-compose logs relay
docker-compose logs remote-agent
docker-compose logs test-client

# Run tests manually inside container
docker-compose exec test-client python3 /app/scripts/e2e_test.py
```

### Permission Issues

```bash
# Ensure proper permissions on mount points
chmod 755 /tmp/remotefs-mount

# For NFS testing, ensure sudo access
sudo echo "Testing sudo access"
```

### Port Conflicts

If you have port conflicts, modify `docker-compose.yml`:

```yaml
ports:
  - "8080:8080"    # Change first port: "8090:8080" 
  - "8081:8081"    # Change first port: "8091:8081"
  - "2049:2049"    # Change first port: "2050:2049"
```

## Performance Expectations

Typical performance metrics on modern hardware:

- **File Write**: 10-50ms per 2KB file
- **File Read**: 5-20ms per 2KB file  
- **Directory List**: 1-10ms for small directories
- **Git Operations**: 100-500ms per operation
- **Overall Test Suite**: 30-120 seconds

## Extending the Tests

### Adding New Tests

1. Add test method to `scripts/e2e_test.py`
2. Follow the existing pattern with `TestResult` return
3. Add the method to `test_methods` list in `run_all_tests()`

### Modifying Mock Project

1. Edit files in `mock-project/`
2. Rebuild containers: `./scripts/setup.sh --force-recreate`

### Custom Configurations

1. Modify config files in `configs/`
2. Restart services: `./scripts/teardown.sh && ./scripts/setup.sh`

## Integration with CI/CD

Example GitHub Actions workflow:

```yaml
name: RemoteFS E2E Tests
on: [push, pull_request]
jobs:
  e2e-tests:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v3
      - name: Run E2E Tests
        run: |
          cd e2e-tests
          ./scripts/setup.sh
          ./scripts/run_tests.sh
          ./scripts/teardown.sh --preserve-results
      - name: Upload Results
        uses: actions/upload-artifact@v3
        with:
          name: test-results
          path: e2e-tests/test-results/
```

## Contributing

When adding new tests or modifying the environment:

1. Ensure all tests pass locally
2. Update this README if adding new features
3. Include appropriate error handling
4. Add logging for debugging
5. Test both success and failure scenarios

---

For questions or issues, please refer to the main RemoteFS documentation or create an issue in the project repository.
