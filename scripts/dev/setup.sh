#!/usr/bin/env bash
set -euo pipefail

echo "Setting up swebash development environment..."

if ! command -v rustup &>/dev/null; then
    echo "Installing Rust toolchain..."
    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
    source "$HOME/.cargo/env"
fi

echo "Syncing toolchain from rust-toolchain.toml (if present)..."
rustup show

echo "Installing clippy and rustfmt..."
rustup component add clippy rustfmt

echo "Setup complete."
