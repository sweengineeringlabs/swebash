# bin/test.ps1 — Build and test swebash (Windows)
param(
    [ValidateSet("engine", "host", "readline", "ai", "all")]
    [string]$Suite = "all"
)

. "$PSScriptRoot\..\lib\common.ps1"
Invoke-Preflight

Write-Host "==> Building engine WASM (required for tests)..."
cargo build --manifest-path "$RepoRoot\features\shell\engine\Cargo.toml" `
    --target wasm32-unknown-unknown --release

function Test-Engine {
    Write-Host "==> Testing engine..."
    cargo test --manifest-path "$RepoRoot\features\shell\engine\Cargo.toml"
}

function Test-Host {
    Write-Host "==> Testing host..."
    cargo test --manifest-path "$RepoRoot\features\shell\host\Cargo.toml"
}

function Test-Readline {
    Write-Host "==> Testing readline..."
    cargo test --manifest-path "$RepoRoot\features\shell\readline\Cargo.toml"
}

function Test-Ai {
    Write-Host "==> Testing ai..."
    cargo test --manifest-path "$RepoRoot\features\ai\Cargo.toml"
}

switch ($Suite) {
    "engine"   { Test-Engine }
    "host"     { Test-Host }
    "readline" { Test-Readline }
    "ai"       { Test-Ai }
    "all"      {
        Test-Engine
        Test-Readline
        Test-Host
        Test-Ai
    }
}

Write-Host "==> Tests complete ($Suite)"
