# Unit tests for bin/build.ps1 (Pester 3.4.0)

$here = Split-Path -Parent $MyInvocation.MyCommand.Path
$RepoRoot = Resolve-Path "$here\..\..\.."

Describe "build.ps1" {
    BeforeEach {
        $script:savedReg = $env:CARGO_REGISTRIES_LOCAL_INDEX
        # Set up a valid registry so Invoke-Preflight passes
        $script:tmpReg = Join-Path $env:TEMP "pester-reg-$(Get-Random)"
        New-Item -ItemType Directory -Path $script:tmpReg -Force | Out-Null
        $env:CARGO_REGISTRIES_LOCAL_INDEX = "file:///$($script:tmpReg -replace '\\','/')"
    }
    AfterEach {
        $env:CARGO_REGISTRIES_LOCAL_INDEX = $script:savedReg
        if (Test-Path $script:tmpReg) { Remove-Item -Recurse -Force $script:tmpReg }
    }

    It "default build is release" {
        Mock cargo { Write-Output "cargo $args" } -Verifiable
        . "$RepoRoot\lib\common.ps1"
        Mock Invoke-Preflight {}

        # Source the script in a controlled way
        $output = powershell -NoProfile -Command @"
            `$env:CARGO_REGISTRIES_LOCAL_INDEX = '$env:CARGO_REGISTRIES_LOCAL_INDEX'
            function cargo { Write-Output "cargo `$args" }
            . '$RepoRoot\lib\common.ps1'
            function Invoke-Preflight {}
            . '$RepoRoot\bin\build.ps1'
"@ 2>&1 | Out-String

        $output | Should Match "release"
    }

    It "Debug flag selects debug profile" {
        $output = powershell -NoProfile -Command @"
            `$env:CARGO_REGISTRIES_LOCAL_INDEX = '$env:CARGO_REGISTRIES_LOCAL_INDEX'
            function cargo { Write-Output "cargo `$args" }
            . '$RepoRoot\lib\common.ps1'
            function Invoke-Preflight {}
            . '$RepoRoot\bin\build.ps1' -Debug
"@ 2>&1 | Out-String

        $output | Should Match "debug"
    }

    It "builds both engine and host" {
        $output = powershell -NoProfile -Command @"
            `$env:CARGO_REGISTRIES_LOCAL_INDEX = '$env:CARGO_REGISTRIES_LOCAL_INDEX'
            function cargo { Write-Output "cargo `$args" }
            . '$RepoRoot\lib\common.ps1'
            function Invoke-Preflight {}
            . '$RepoRoot\bin\build.ps1'
"@ 2>&1 | Out-String

        $output | Should Match "engine"
        $output | Should Match "host"
    }
}
