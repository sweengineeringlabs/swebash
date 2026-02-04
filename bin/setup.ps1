# bin/setup.ps1 — One-time environment setup for swebash (Windows)
. "$PSScriptRoot\..\lib\common.ps1"

Write-Host "==> Installing wasm32-unknown-unknown target..."
rustup target add wasm32-unknown-unknown

# ── Verify local registry ───────────────────────────────────────────
$RegPath = "C:\Users\elvis\.cargo\registry.local\index"

if (Test-Path $RegPath) {
    Write-Host "==> Local registry found at $RegPath"
} else {
    Write-Warning "Local registry not found at $RegPath"
    Write-Warning "You may need to set up the rustratify registry manually."
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
        Write-Warning ".env.example not found — skipping .env creation"
    }
} else {
    Write-Host "==> .env already exists"
}

# ── Summary ──────────────────────────────────────────────────────────
Write-Host ""
Write-Host "Setup complete!"
Write-Host "  Registry: $RegUrl"
Write-Host "  .env:     $EnvFile"
Write-Host ""
Write-Host "Next steps:"
Write-Host "  1. Edit .env with your API keys"
Write-Host "  2. Run: .\sbh build"
