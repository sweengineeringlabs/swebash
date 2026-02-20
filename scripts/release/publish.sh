#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"

echo "Publishing swebash workspace crates..."

# Publish in dependency order
PUBLISH_ORDER=(
    "$PROJECT_ROOT/swe-readline"
    "$PROJECT_ROOT/features/shell/readline"
    "$PROJECT_ROOT/features/shell/engine"
    "$PROJECT_ROOT/features/ai"
    "$PROJECT_ROOT/features/test"
    "$PROJECT_ROOT/features/shell/host"
)

for crate_dir in "${PUBLISH_ORDER[@]}"; do
    if [[ -f "$crate_dir/Cargo.toml" ]]; then
        echo "Publishing $(basename "$crate_dir")..."
        cargo publish --manifest-path "$crate_dir/Cargo.toml"
        echo "Waiting for crates.io to index..."
        sleep 10
    fi
done

echo "Publish complete."
