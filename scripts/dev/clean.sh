#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"

echo "Cleaning swebash workspace..."
cargo clean --manifest-path "$PROJECT_ROOT/Cargo.toml"
echo "Clean complete."
