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
