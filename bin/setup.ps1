# bin/setup.ps1 — One-time environment setup for swebash (Windows)
. "$PSScriptRoot\..\lib\common.ps1"

# ── Check prerequisites ──────────────────────────────────────────────
Write-Host "==> Checking prerequisites..."

if (-not (Get-Command rustup -ErrorAction SilentlyContinue)) {
    Write-Error "rustup not found. Install from https://rustup.rs"
    exit 1
}

if (-not (Get-Command cargo -ErrorAction SilentlyContinue)) {
    Write-Error "cargo not found. Install Rust via rustup."
    exit 1
}

Write-Host "  rustup: $(rustup --version 2>&1 | Select-Object -First 1)"
Write-Host "  cargo:  $(cargo --version)"

# ── Install WASM target ─────────────────────────────────────────────
Write-Host "==> Installing wasm32-unknown-unknown target..."
rustup target add wasm32-unknown-unknown

# ── Verify local registry ───────────────────────────────────────────
$RegPath = Join-Path $env:USERPROFILE ".cargo\registry.local\index"

if (Test-Path $RegPath) {
    Write-Host "==> Local registry found at $RegPath"
} else {
    Write-Error "Local registry not found at $RegPath"
    Write-Error "Set up the rustratify registry before running setup."
    exit 1
}

# ── Set CARGO_REGISTRIES_LOCAL_INDEX (user env var) ──────────────────
$RegUrl = "file:///$($RegPath -replace '\\','/')"
$CurrentVal = [Environment]::GetEnvironmentVariable("CARGO_REGISTRIES_LOCAL_INDEX", "User")

if (-not $CurrentVal) {
    [Environment]::SetEnvironmentVariable("CARGO_REGISTRIES_LOCAL_INDEX", $RegUrl, "User")
    $env:CARGO_REGISTRIES_LOCAL_INDEX = $RegUrl
    Write-Host "==> Set CARGO_REGISTRIES_LOCAL_INDEX as user environment variable"
} else {
    Write-Host "==> CARGO_REGISTRIES_LOCAL_INDEX already set"
}

# ── Copy .env.example → .env ────────────────────────────────────────
$EnvFile = Join-Path $RepoRoot ".env"
$EnvExample = Join-Path $RepoRoot ".env.example"

if (-not (Test-Path $EnvFile)) {
    if (Test-Path $EnvExample) {
        Copy-Item $EnvExample $EnvFile
        Write-Host "==> Copied .env.example -> .env (edit API keys before running)"
    } else {
        Write-Warning ".env.example not found -- skipping .env creation"
    }
} else {
    Write-Host "==> .env already exists"
}

# ── Verify setup ─────────────────────────────────────────────────────
Write-Host ""
Verify-Registry

# ── Summary ──────────────────────────────────────────────────────────
Write-Host ""
Write-Host "Setup complete!"
Write-Host "  Registry: $RegUrl"
Write-Host "  .env:     $EnvFile"
Write-Host ""
Write-Host "Next steps:"
Write-Host "  1. Edit .env with your API keys"
Write-Host "  2. Run: .\sbh build"
