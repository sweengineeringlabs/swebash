#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"

echo "Running clippy..."
cargo clippy --workspace --manifest-path "$PROJECT_ROOT/Cargo.toml" -- -D warnings

echo "Checking formatting..."
cargo fmt --all --manifest-path "$PROJECT_ROOT/Cargo.toml" -- --check

echo "Lint complete."
