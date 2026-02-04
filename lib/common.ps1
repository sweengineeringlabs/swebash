# lib/common.ps1 — shared helpers for swebash PowerShell scripts

$ErrorActionPreference = "Stop"
$RepoRoot = Split-Path -Parent $PSScriptRoot

# ── Registry setup ───────────────────────────────────────────────────
function Ensure-Registry {
    if (-not $env:CARGO_REGISTRIES_LOCAL_INDEX) {
        $regPath = "C:\Users\elvis\.cargo\registry.local\index"
        if (Test-Path $regPath) {
            $env:CARGO_REGISTRIES_LOCAL_INDEX = "file:///$($regPath -replace '\\','/')"
        } else {
            Write-Error "Local registry not found at $regPath"
            exit 1
        }
    }
}

# ── Load .env ────────────────────────────────────────────────────────
function Load-EnvFile {
    param([string]$Path = (Join-Path $RepoRoot ".env"))
    if (Test-Path $Path) {
        Get-Content $Path | ForEach-Object {
            if ($_ -match '^\s*([^#]\S+?)\s*=\s*"?(.+?)"?\s*$') {
                [Environment]::SetEnvironmentVariable($Matches[1], $Matches[2], "Process")
            }
        }
    }
}
