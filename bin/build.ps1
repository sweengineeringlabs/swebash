# bin/build.ps1 — Build engine (WASM) and host for swebash (Windows)
param(
    [switch]$Debug
)

. "$PSScriptRoot\..\lib\common.ps1"
Invoke-Preflight

if ($Debug) {
    $BuildFlag = @()
    $ProfileLabel = "debug"
} else {
    $BuildFlag = @("--release")
    $ProfileLabel = "release"
}

Write-Host "==> Building engine (wasm32, $ProfileLabel)..."
cargo build --manifest-path "$RepoRoot\features\shell\engine\Cargo.toml" `
    --target wasm32-unknown-unknown @BuildFlag

Write-Host "==> Building host ($ProfileLabel)..."
cargo build --manifest-path "$RepoRoot\features\shell\host\Cargo.toml" @BuildFlag

Write-Host "==> Build complete ($ProfileLabel)"
