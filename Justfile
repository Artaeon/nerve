# Nerve — development task runner

# Show all recipes
default:
    @just --list

# Build in debug mode
build:
    cargo build

# Build release binary
release:
    cargo build --release

# Run all checks (mirrors CI)
check: fmt-check lint test
    cargo check --all-targets

# Run tests
test:
    cargo test --all-targets

# Run clippy
lint:
    cargo clippy --all-targets -- -D warnings

# Check formatting
fmt-check:
    cargo fmt --all -- --check

# Format code
fmt:
    cargo fmt --all

# Security audit
audit:
    cargo audit

# Install locally
install: release
    cp target/release/nerve ~/.local/bin/
    @echo "Installed nerve to ~/.local/bin/"

# Clean build artifacts
clean:
    cargo clean

# Run nerve in debug mode
run *ARGS:
    cargo run -- {{ARGS}}

# Count lines of code
loc:
    @find src -name '*.rs' | xargs wc -l | tail -1
