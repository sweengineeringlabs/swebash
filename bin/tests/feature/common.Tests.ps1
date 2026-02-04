# Unit tests for lib/common.ps1 (Pester 3.4.0)

$here = Split-Path -Parent $MyInvocation.MyCommand.Path
$RepoRoot = Resolve-Path "$here\..\..\.."

# Source the module under test
. "$RepoRoot\lib\common.ps1"

Describe "Verify-Registry" {
    BeforeEach {
        $script:savedReg = $env:CARGO_REGISTRIES_LOCAL_INDEX
    }
    AfterEach {
        $env:CARGO_REGISTRIES_LOCAL_INDEX = $script:savedReg
    }

    It "exits 1 when env var is unset" {
        $env:CARGO_REGISTRIES_LOCAL_INDEX = $null
        { Verify-Registry } | Should Throw
    }

    It "exits 1 when path is missing" {
        $env:CARGO_REGISTRIES_LOCAL_INDEX = "file:///C:/nonexistent/path/does/not/exist"
        { Verify-Registry } | Should Throw
    }

    It "prints (ok) for valid path" {
        $tmpDir = Join-Path $env:TEMP "pester-registry-$(Get-Random)"
        New-Item -ItemType Directory -Path $tmpDir -Force | Out-Null
        $env:CARGO_REGISTRIES_LOCAL_INDEX = "file:///$($tmpDir -replace '\\','/')"
        $output = Verify-Registry 4>&1 6>&1 | Out-String
        $output | Should Match "\(ok\)"
        Remove-Item -Recurse -Force $tmpDir
    }
}

Describe "Ensure-Registry" {
    BeforeEach {
        $script:savedReg = $env:CARGO_REGISTRIES_LOCAL_INDEX
    }
    AfterEach {
        $env:CARGO_REGISTRIES_LOCAL_INDEX = $script:savedReg
    }

    It "sets env var when valid registry path exists" {
        $env:CARGO_REGISTRIES_LOCAL_INDEX = $null
        # Create a fake registry path
        $fakeReg = Join-Path $env:USERPROFILE ".cargo\registry.local\index"
        if (Test-Path $fakeReg) {
            Ensure-Registry
            $env:CARGO_REGISTRIES_LOCAL_INDEX | Should Not BeNullOrEmpty
        } else {
            # If no real registry, Ensure-Registry will throw — that's expected
            { Ensure-Registry } | Should Throw
        }
    }

    It "syncs Cargo.lock when registry path is stale" {
        $tmpDir = Join-Path $env:TEMP "pester-cargolock-$(Get-Random)"
        New-Item -ItemType Directory -Path $tmpDir -Force | Out-Null
        $cargoLock = Join-Path $tmpDir "Cargo.lock"
        [System.IO.File]::WriteAllText($cargoLock, @"
[[package]]
name = "some-crate"
version = "0.1.0"
source = "registry+file:///mnt/c/Users/olduser/.cargo/registry.local/index"

[[package]]
name = "another-crate"
version = "0.2.0"
source = "registry+file:///mnt/c/Users/olduser/.cargo/registry.local/index"
"@)
        $env:CARGO_REGISTRIES_LOCAL_INDEX = "file:///C:/Users/newuser/.cargo/registry.local/index"
        $savedRepoRoot = $RepoRoot
        $script:RepoRoot = $tmpDir
        Ensure-Registry
        $script:RepoRoot = $savedRepoRoot
        $content = [System.IO.File]::ReadAllText($cargoLock)
        $content | Should Match "registry\+file:///C:/Users/newuser/.cargo/registry\.local/index"
        $content | Should Not Match "registry\+file:///mnt/c/Users/olduser"
        Remove-Item -Recurse -Force $tmpDir
    }

    It "skips Cargo.lock when registry path already matches" {
        $tmpDir = Join-Path $env:TEMP "pester-cargolock-$(Get-Random)"
        New-Item -ItemType Directory -Path $tmpDir -Force | Out-Null
        $cargoLock = Join-Path $tmpDir "Cargo.lock"
        [System.IO.File]::WriteAllText($cargoLock, @"
[[package]]
name = "some-crate"
version = "0.1.0"
source = "registry+file:///C:/Users/elvis/.cargo/registry.local/index"
"@)
        $env:CARGO_REGISTRIES_LOCAL_INDEX = "file:///C:/Users/elvis/.cargo/registry.local/index"
        $savedRepoRoot = $RepoRoot
        $script:RepoRoot = $tmpDir
        $output = Ensure-Registry 4>&1 6>&1 | Out-String
        $script:RepoRoot = $savedRepoRoot
        $output | Should Not Match "synced registry paths"
        Remove-Item -Recurse -Force $tmpDir
    }

    It "no-ops when Cargo.lock is missing" {
        $tmpDir = Join-Path $env:TEMP "pester-cargolock-$(Get-Random)"
        New-Item -ItemType Directory -Path $tmpDir -Force | Out-Null
        # No Cargo.lock created
        $env:CARGO_REGISTRIES_LOCAL_INDEX = "file:///C:/Users/elvis/.cargo/registry.local/index"
        $savedRepoRoot = $RepoRoot
        $script:RepoRoot = $tmpDir
        { Ensure-Registry } | Should Not Throw
        $script:RepoRoot = $savedRepoRoot
        Remove-Item -Recurse -Force $tmpDir
    }

    It "preserves crates.io entries in Cargo.lock" {
        $tmpDir = Join-Path $env:TEMP "pester-cargolock-$(Get-Random)"
        New-Item -ItemType Directory -Path $tmpDir -Force | Out-Null
        $cargoLock = Join-Path $tmpDir "Cargo.lock"
        [System.IO.File]::WriteAllText($cargoLock, @"
[[package]]
name = "serde"
version = "1.0.0"
source = "registry+https://github.com/rust-lang/crates.io-index"

[[package]]
name = "local-crate"
version = "0.1.0"
source = "registry+file:///mnt/c/Users/old/.cargo/registry.local/index"
"@)
        $env:CARGO_REGISTRIES_LOCAL_INDEX = "file:///C:/Users/new/.cargo/registry.local/index"
        $savedRepoRoot = $RepoRoot
        $script:RepoRoot = $tmpDir
        Ensure-Registry
        $script:RepoRoot = $savedRepoRoot
        $content = [System.IO.File]::ReadAllText($cargoLock)
        $content | Should Match "registry\+https://github\.com/rust-lang/crates\.io-index"
        $content | Should Match "registry\+file:///C:/Users/new/.cargo/registry\.local/index"
        Remove-Item -Recurse -Force $tmpDir
    }
}

Describe "Load-EnvFile" {
    It "sets variables from .env file" {
        $tmpFile = Join-Path $env:TEMP "pester-env-$(Get-Random).env"
        Set-Content -Path $tmpFile -Value 'PESTER_TEST_VAR=hello_pester'
        $savedVal = [Environment]::GetEnvironmentVariable("PESTER_TEST_VAR", "Process")

        Load-EnvFile -Path $tmpFile
        $env:PESTER_TEST_VAR | Should Be "hello_pester"

        # Cleanup
        [Environment]::SetEnvironmentVariable("PESTER_TEST_VAR", $savedVal, "Process")
        Remove-Item -Force $tmpFile
    }

    It "ignores comment lines" {
        $tmpFile = Join-Path $env:TEMP "pester-env-$(Get-Random).env"
        Set-Content -Path $tmpFile -Value @(
            "# This is a comment"
            "PESTER_COMMENT_TEST=works"
        )
        $savedVal = [Environment]::GetEnvironmentVariable("PESTER_COMMENT_TEST", "Process")

        Load-EnvFile -Path $tmpFile
        $env:PESTER_COMMENT_TEST | Should Be "works"

        [Environment]::SetEnvironmentVariable("PESTER_COMMENT_TEST", $savedVal, "Process")
        Remove-Item -Force $tmpFile
    }

    It "handles quoted values" {
        $tmpFile = Join-Path $env:TEMP "pester-env-$(Get-Random).env"
        Set-Content -Path $tmpFile -Value 'PESTER_QUOTED_TEST="quoted value"'
        $savedVal = [Environment]::GetEnvironmentVariable("PESTER_QUOTED_TEST", "Process")

        Load-EnvFile -Path $tmpFile
        $env:PESTER_QUOTED_TEST | Should Be "quoted value"

        [Environment]::SetEnvironmentVariable("PESTER_QUOTED_TEST", $savedVal, "Process")
        Remove-Item -Force $tmpFile
    }

    It "no-ops when file is missing" {
        { Load-EnvFile -Path "C:\nonexistent\path\.env" } | Should Not Throw
    }
}
