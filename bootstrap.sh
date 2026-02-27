#!/usr/bin/env bash
# bootstrap.sh - Set up development environment for swebash
set -euo pipefail

echo "=== swebash bootstrap ==="

# Check for required tools
command -v cargo >/dev/null 2>&1 || { echo "ERROR: cargo not found. Install Rust: https://rustup.rs"; exit 1; }

# Install cargo tools if not present
if ! command -v cargo-deny >/dev/null 2>&1; then
    echo "Installing cargo-deny..."
    cargo install cargo-deny
fi

# Build the project
echo "Building workspace..."
cargo build --workspace

echo "=== Bootstrap complete ==="
