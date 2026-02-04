# Unit tests for bin/run.ps1 (Pester 3.4.0)

$here = Split-Path -Parent $MyInvocation.MyCommand.Path
$RepoRoot = Resolve-Path "$here\..\..\.."

Describe "run.ps1" {
    BeforeEach {
        $script:savedReg = $env:CARGO_REGISTRIES_LOCAL_INDEX
        $script:tmpReg = Join-Path $env:TEMP "pester-reg-$(Get-Random)"
        New-Item -ItemType Directory -Path $script:tmpReg -Force | Out-Null
        $env:CARGO_REGISTRIES_LOCAL_INDEX = "file:///$($script:tmpReg -replace '\\','/')"
    }
    AfterEach {
        $env:CARGO_REGISTRIES_LOCAL_INDEX = $script:savedReg
        if (Test-Path $script:tmpReg) { Remove-Item -Recurse -Force $script:tmpReg }
    }

    It "default profile is debug" {
        $output = powershell -NoProfile -Command @"
            `$ErrorActionPreference = 'Continue'
            `$env:CARGO_REGISTRIES_LOCAL_INDEX = '$env:CARGO_REGISTRIES_LOCAL_INDEX'
            function cargo { Write-Output "cargo `$args" }
            . '$RepoRoot\lib\common.ps1'
            function Invoke-Preflight {}
            function Load-EnvFile {}
            try { . '$RepoRoot\bin\run.ps1' } catch {}
"@ 2>&1 | Out-String

        $output | Should Match "debug"
    }

    It "Release flag selects release" {
        $output = powershell -NoProfile -Command @"
            `$ErrorActionPreference = 'Continue'
            `$env:CARGO_REGISTRIES_LOCAL_INDEX = '$env:CARGO_REGISTRIES_LOCAL_INDEX'
            function cargo { Write-Output "cargo `$args" }
            . '$RepoRoot\lib\common.ps1'
            function Invoke-Preflight {}
            function Load-EnvFile {}
            try { . '$RepoRoot\bin\run.ps1' -Release } catch {}
"@ 2>&1 | Out-String

        $output | Should Match "release"
    }

    It "triggers build when binaries are missing" {
        $output = powershell -NoProfile -Command @"
            `$ErrorActionPreference = 'Continue'
            `$env:CARGO_REGISTRIES_LOCAL_INDEX = '$env:CARGO_REGISTRIES_LOCAL_INDEX'
            function cargo { Write-Output "cargo `$args" }
            . '$RepoRoot\lib\common.ps1'
            function Invoke-Preflight {}
            function Load-EnvFile {}
            try { . '$RepoRoot\bin\run.ps1' } catch {}
"@ 2>&1 | Out-String

        $output | Should Match "not found|building"
    }
}
