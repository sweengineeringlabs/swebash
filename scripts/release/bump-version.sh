#!/usr/bin/env bash
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/../.." && pwd)"

NEW_VERSION="${1:-}"
if [[ -z "$NEW_VERSION" ]]; then
    echo "Usage: $0 <version>"
    echo "Example: $0 0.2.0"
    exit 1
fi

MEMBER_TOMLS=(
    "$PROJECT_ROOT/swe-readline/Cargo.toml"
    "$PROJECT_ROOT/features/shell/engine/Cargo.toml"
    "$PROJECT_ROOT/features/shell/host/Cargo.toml"
    "$PROJECT_ROOT/features/shell/readline/Cargo.toml"
    "$PROJECT_ROOT/features/ai/Cargo.toml"
    "$PROJECT_ROOT/features/test/Cargo.toml"
)

for toml in "${MEMBER_TOMLS[@]}"; do
    if [[ -f "$toml" ]]; then
        sed -i "s/^version = \".*\"/version = \"$NEW_VERSION\"/" "$toml"
        echo "Updated $toml"
    fi
done

echo "Bumped all workspace members to $NEW_VERSION."
