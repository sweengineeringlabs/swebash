# lib/common.ps1 — shared helpers for swebash PowerShell scripts

$ErrorActionPreference = "Stop"
$RepoRoot = Split-Path -Parent $PSScriptRoot

# ── Preflight checks ──────────────────────────────────────────────────
function Invoke-Preflight {
    Ensure-Registry
    Verify-Registry
}

# ── Registry setup ───────────────────────────────────────────────────
function Ensure-Registry {
    if (-not $env:CARGO_REGISTRIES_LOCAL_INDEX) {
        $regPath = Join-Path $env:USERPROFILE ".cargo\registry.local\index"
        if (Test-Path $regPath) {
            $env:CARGO_REGISTRIES_LOCAL_INDEX = "file:///$($regPath -replace '\\','/')"
        } else {
            Write-Error "Local registry not found at $regPath"
            exit 1
        }
    }
}

# ── Registry verification ─────────────────────────────────────────────
function Verify-Registry {
    if (-not $env:CARGO_REGISTRIES_LOCAL_INDEX) {
        Write-Error "CARGO_REGISTRIES_LOCAL_INDEX is not set"
        exit 1
    }

    # Strip file:/// prefix to get the filesystem path
    $indexPath = $env:CARGO_REGISTRIES_LOCAL_INDEX -replace '^file:///', ''
    if (-not (Test-Path $indexPath)) {
        Write-Error "Local registry index not found at $indexPath"
        Write-Error "  CARGO_REGISTRIES_LOCAL_INDEX=$env:CARGO_REGISTRIES_LOCAL_INDEX"
        exit 1
    }

    Write-Host "==> Registry: $env:CARGO_REGISTRIES_LOCAL_INDEX (ok)"
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
