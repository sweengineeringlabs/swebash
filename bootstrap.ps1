# bootstrap.ps1 - Set up development environment for swebash (Windows)
$ErrorActionPreference = "Stop"

Write-Host "=== swebash bootstrap ===" -ForegroundColor Cyan

# Check for required tools
if (-not (Get-Command cargo -ErrorAction SilentlyContinue)) {
    Write-Host "ERROR: cargo not found. Install Rust: https://rustup.rs" -ForegroundColor Red
    exit 1
}

# Install cargo tools if not present
if (-not (Get-Command cargo-deny -ErrorAction SilentlyContinue)) {
    Write-Host "Installing cargo-deny..."
    cargo install cargo-deny
}

# Build the project
Write-Host "Building workspace..."
cargo build --workspace

Write-Host "=== Bootstrap complete ===" -ForegroundColor Green
