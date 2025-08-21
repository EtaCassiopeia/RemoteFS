.PHONY: all build test clean install release check fmt clippy doc

# Default target
all: build

# Build all components in debug mode
build:
	cargo build --workspace

# Build release binaries
release:
	cargo build --workspace --release

# Run tests
test:
	cargo test --workspace

# Run tests with output
test-verbose:
	cargo test --workspace -- --nocapture

# Clean build artifacts
clean:
	cargo clean

# Install binaries to local cargo bin
install:
	cargo install --path remotefs-client
	cargo install --path remotefs-agent
	cargo install --path remotefs-relay

# Check code without building
check:
	cargo check --workspace

# Format code
fmt:
	cargo fmt --all

# Run clippy linter
clippy:
	cargo clippy --workspace --all-targets --all-features

# Generate documentation
doc:
	cargo doc --workspace --no-deps --open

# Run all quality checks
quality: fmt clippy test

# Development setup - install tools and create configs
dev-setup:
	rustup component add rustfmt clippy
	@echo "Creating default configurations..."
	@mkdir -p ~/.config/remotefs
	@echo "Development setup complete!"

# Start local development environment
dev-start: build
	@echo "Starting development environment..."
	@echo "1. Start relay server in terminal 1:"
	@echo "   RUST_LOG=debug target/debug/remotefs-relay"
	@echo ""
	@echo "2. Start agent in terminal 2:"  
	@echo "   RUST_LOG=debug target/debug/remotefs-agent"
	@echo ""
	@echo "3. Start client in terminal 3:"
	@echo "   RUST_LOG=debug target/debug/remotefs-client"

# Create example configurations
example-configs:
	@echo "Creating example configuration files..."
	@mkdir -p examples/config
	@echo "# Example client configuration" > examples/config/client.toml
	@echo "# Example agent configuration" > examples/config/agent.toml  
	@echo "# Example relay configuration" > examples/config/relay.toml
	@echo "Example configurations created in examples/config/"

# Run security audit
audit:
	cargo audit

# Update dependencies
update:
	cargo update

# Show project statistics
stats:
	@echo "=== RemoteFS Project Statistics ==="
	@echo "Lines of code (Rust):"
	@find . -name "*.rs" -not -path "./target/*" | xargs wc -l | tail -n 1
	@echo ""
	@echo "Crate information:"
	@cargo tree --workspace --depth 1
	@echo ""
	@echo "Binary sizes (release mode):"
	@if [ -f target/release/remotefs-client ]; then \
		ls -lh target/release/remotefs-* | awk '{print $$9 " - " $$5}'; \
	else \
		echo "No release binaries found. Run 'make release' first."; \
	fi

# Create deployment package
package: release
	@echo "Creating deployment package..."
	@mkdir -p dist
	@cp target/release/remotefs-client dist/
	@cp target/release/remotefs-agent dist/
	@cp target/release/remotefs-relay dist/
	@cp README.md ARCHITECTURE.md dist/
	@tar -czf remotefs-$(shell date +%Y%m%d).tar.gz -C dist .
	@echo "Package created: remotefs-$(shell date +%Y%m%d).tar.gz"

# Help target
help:
	@echo "RemoteFS Makefile Commands:"
	@echo ""
	@echo "Building:"
	@echo "  build          - Build debug binaries"
	@echo "  release        - Build release binaries"  
	@echo "  install        - Install binaries locally"
	@echo ""
	@echo "Testing & Quality:"
	@echo "  test           - Run tests"
	@echo "  test-verbose   - Run tests with output"
	@echo "  check          - Check code without building"
	@echo "  clippy         - Run linter"
	@echo "  fmt            - Format code"
	@echo "  quality        - Run all quality checks"
	@echo "  audit          - Security audit"
	@echo ""
	@echo "Development:"
	@echo "  dev-setup      - Initial development setup"
	@echo "  dev-start      - Show commands to start dev environment"
	@echo "  example-configs - Create example configuration files"
	@echo ""
	@echo "Utilities:"
	@echo "  clean          - Clean build artifacts"
	@echo "  doc            - Generate documentation"
	@echo "  update         - Update dependencies"
	@echo "  stats          - Show project statistics"
	@echo "  package        - Create deployment package"
	@echo "  help           - Show this help message"
