# RemoteFS E2E Tests - Quick Start

## TL;DR

```bash
# 1. Setup the environment
cd e2e-tests
./scripts/setup.sh

# 2. Run the tests  
./scripts/run_tests.sh

# 3. Cleanup when done
./scripts/teardown.sh
```

## What This Tests

This E2E test suite simulates a **remote macOS development machine** scenario where:

- 🖥️ You have a **remote macOS machine** with source code (simulated in Docker)
- 🌐 A **relay server** handles routing and authentication  
- 💻 Your **local machine** mounts the remote filesystem
- 🤖 **AI/LLM operations** like code editing, git operations, file searching

Perfect for testing scenarios like:
- **Warp Terminal** operations on remote codebases
- **AI coding assistants** modifying files on remote machines
- **Distributed development** workflows
- **Performance** of file operations over the network

## Test Environment

```
Local Machine                 Remote macOS Machine
┌─────────────┐              ┌─────────────────────┐
│ Test Client │──────────────│ Relay Server        │
│             │              │                     │
│ - Mounts FS │              │ ┌─────────────────┐ │
│ - Runs Tests│              │ │ Remote Agent    │ │
│ - Benchmarks│              │ │ - Git Repo      │ │
└─────────────┘              │ │ - Source Code   │ │
                             │ │ - Dev Tools     │ │
                             │ └─────────────────┘ │
                             └─────────────────────┘
```

## What Gets Tested

✅ **File Operations**: Create, read, update, delete files  
✅ **Directory Operations**: Create nested directories, list contents  
✅ **Git Operations**: Branch creation, commits, status checks  
✅ **Code Modifications**: LLM-style code editing and validation  
✅ **File Discovery**: Find files, grep through source code  
✅ **Performance**: Measure I/O speeds and concurrent operations  

## Sample Output

```
[INFO] Starting RemoteFS End-to-End Tests
[INFO] Waiting for filesystem mount...
[INFO] Running test_basic_connectivity...
test_basic_connectivity: PASS (1.2s)
[INFO] Running test_git_operations...
test_git_operations: PASS (4.8s)
[INFO] Running test_code_modifications...
test_code_modifications: PASS (8.3s)

Test Summary:
Total Tests: 7
Passed: 7  
Failed: 0
Success Rate: 100.0%
Total Duration: 42.1s
```

## Troubleshooting

**Services won't start?**
```bash
docker --version  # Ensure Docker is installed
./scripts/setup.sh help  # Check options
```

**Tests failing?**
```bash
./scripts/run_tests.sh --debug  # Get detailed logs
docker-compose logs relay      # Check individual services
```

**Want to explore manually?**
```bash
# Access the remote filesystem
ls /tmp/remotefs-mount

# Run commands on the "remote macOS machine"
docker-compose exec remote-agent bash
```

For full documentation, see [README.md](README.md)
