# bin/run.ps1 — Build (if needed) and run swebash (Windows)
param(
    [switch]$Release,
    [switch]$Debug
)

. "$PSScriptRoot\..\lib\common.ps1"
Invoke-Preflight
Load-EnvFile

if ($Release) {
    $BuildFlag = @("--release")
    $ProfileDir = "release"
} else {
    $BuildFlag = @()
    $ProfileDir = "debug"
}

$TargetDir = Join-Path $RepoRoot "target"
$WasmBin = Join-Path $TargetDir "wasm32-unknown-unknown\$ProfileDir\engine.wasm"
$HostBin = Join-Path $TargetDir "$ProfileDir\swebash.exe"

if (-not (Test-Path $WasmBin) -or -not (Test-Path $HostBin)) {
    Write-Host "==> Binaries not found, building ($ProfileDir)..."
    cargo build --manifest-path "$RepoRoot\features\shell\engine\Cargo.toml" `
        --target wasm32-unknown-unknown @BuildFlag
    cargo build --manifest-path "$RepoRoot\features\shell\host\Cargo.toml" @BuildFlag
}

Write-Host "==> Launching swebash ($ProfileDir)..."
& $HostBin
