# lib/common.ps1 — shared helpers for swebash PowerShell scripts

$ErrorActionPreference = "Stop"
$RepoRoot = Split-Path -Parent $PSScriptRoot

# ── Preflight checks ──────────────────────────────────────────────────
function Invoke-Preflight {
    Load-EnvFile
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

    # Sync config.json dl path to match current platform
    $indexUrl = $env:CARGO_REGISTRIES_LOCAL_INDEX
    $indexPath = $indexUrl -replace '^file:///', ''
    $configJson = Join-Path $indexPath "config.json"
    if (Test-Path $configJson) {
        $registryBaseUrl = $indexUrl -replace '/index$', ''
        $expectedDl = "$registryBaseUrl/crates/{crate}/{version}/download"
        $config = Get-Content $configJson -Raw | ConvertFrom-Json
        if ($config.dl -ne $expectedDl) {
            $config.dl = $expectedDl
            $config | ConvertTo-Json | Set-Content $configJson -NoNewline
            git -C $indexPath add config.json
            git -C $indexPath commit -m "sync dl path for windows" --quiet
        }
    }

    # Sync Cargo.lock registry paths to match current platform
    $cargoLock = Join-Path $RepoRoot "Cargo.lock"
    if (Test-Path $cargoLock) {
        $expectedUrl = "registry+$($env:CARGO_REGISTRIES_LOCAL_INDEX)"
        $match = Select-String -Path $cargoLock -Pattern 'registry\+file:///[^"]*registry\.local/index' |
                 Select-Object -First 1
        if ($match) {
            $oldUrl = $match.Matches[0].Value
            if ($oldUrl -ne $expectedUrl) {
                $content = [System.IO.File]::ReadAllText($cargoLock)
                $content = $content.Replace($oldUrl, $expectedUrl)
                [System.IO.File]::WriteAllText($cargoLock, $content)
                Write-Host "==> Cargo.lock: synced registry paths for windows"
            }
        }
    }

    # Sync registry index metadata to match current platform
    if (Test-Path $indexPath) {
        $expectedRegistry = $env:CARGO_REGISTRIES_LOCAL_INDEX
        $metaMatch = Get-ChildItem -Recurse -File $indexPath |
                     Where-Object { $_.FullName -notlike '*\.git\*' } |
                     Select-String -Pattern 'file:///[^"]*registry\.local/index' |
                     Select-Object -First 1
        if ($metaMatch) {
            $oldRegistry = $metaMatch.Matches[0].Value
            if ($oldRegistry -ne $expectedRegistry) {
                Get-ChildItem -Recurse -File $indexPath |
                    Where-Object { $_.FullName -notlike '*\.git\*' } |
                    ForEach-Object {
                        $content = [System.IO.File]::ReadAllText($_.FullName)
                        if ($content.Contains($oldRegistry)) {
                            $content = $content.Replace($oldRegistry, $expectedRegistry)
                            [System.IO.File]::WriteAllText($_.FullName, $content)
                        }
                    }
                git -C $indexPath add -A
                git -C $indexPath commit -m "sync registry metadata paths for windows" --quiet
                # Delete Cargo's index clone so it re-fetches from updated source
                $cargoIndexCache = Join-Path $env:USERPROFILE ".cargo\registry\index"
                if (Test-Path $cargoIndexCache) {
                    Get-ChildItem -Directory $cargoIndexCache |
                        Where-Object { $_.Name -notlike '*crates.io*' } |
                        ForEach-Object { Remove-Item -Recurse -Force $_.FullName }
                }
                Write-Host "==> Registry index: synced metadata paths for windows"
            }
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
