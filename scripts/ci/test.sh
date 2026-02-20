#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"

echo "Running swebash workspace tests..."
cargo test --workspace --manifest-path "$PROJECT_ROOT/Cargo.toml"
echo "Tests complete."
