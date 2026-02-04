# E2E tests for sbh.ps1 entrypoint (Pester 3.4.0)

$here = Split-Path -Parent $MyInvocation.MyCommand.Path
$RepoRoot = Resolve-Path "$here\..\..\.."
$Sbh = Join-Path $RepoRoot "sbh.ps1"

Describe "sbh.ps1 help and usage" {
    It "--help shows usage and exits 0" {
        $output = powershell -NoProfile -File $Sbh --help 2>&1 | Out-String
        $LASTEXITCODE | Should Be 0
        $output | Should Match "Usage"
    }

    It "-h shows usage and exits 0" {
        $output = powershell -NoProfile -File $Sbh -h 2>&1 | Out-String
        $LASTEXITCODE | Should Be 0
        $output | Should Match "Usage"
    }

    It "help command shows usage and exits 0" {
        $output = powershell -NoProfile -File $Sbh help 2>&1 | Out-String
        $LASTEXITCODE | Should Be 0
        $output | Should Match "Usage"
    }

    It "no args shows usage and exits 0" {
        $output = powershell -NoProfile -File $Sbh 2>&1 | Out-String
        $LASTEXITCODE | Should Be 0
        $output | Should Match "Usage"
    }
}

Describe "sbh.ps1 unknown command" {
    It "unknown command shows usage and exits 1" {
        $output = powershell -NoProfile -Command @"
            & '$Sbh' 'totally_invalid_command'
            exit `$LASTEXITCODE
"@ 2>&1 | Out-String
        $LASTEXITCODE | Should Be 1
        $output | Should Match "Usage"
    }
}

Describe "sbh.ps1 help text completeness" {
    BeforeAll {
        $script:helpOutput = powershell -NoProfile -File $Sbh --help 2>&1 | Out-String
    }

    It "lists all commands" {
        $script:helpOutput | Should Match "setup"
        $script:helpOutput | Should Match "build"
        $script:helpOutput | Should Match "run"
        $script:helpOutput | Should Match "test"
    }

    It "lists all test suites" {
        $script:helpOutput | Should Match "engine"
        $script:helpOutput | Should Match "host"
        $script:helpOutput | Should Match "readline"
        $script:helpOutput | Should Match "ai"
        $script:helpOutput | Should Match "scripts"
    }
}

Describe "sbh.ps1 command routing" {
    It "dispatches to correct bin script via stub" {
        $tmpDir = Join-Path $env:TEMP "pester-sbh-$(Get-Random)"
        New-Item -ItemType Directory -Path "$tmpDir\bin" -Force | Out-Null

        # Create a stub build.ps1
        Set-Content -Path "$tmpDir\bin\build.ps1" -Value 'Write-Output "DISPATCH_OK:build"'

        # Create a patched sbh.ps1
        $sbhContent = Get-Content $Sbh -Raw
        $sbhContent = $sbhContent -replace '\$RepoRoot = \$PSScriptRoot', "`$RepoRoot = '$tmpDir'"
        Set-Content -Path "$tmpDir\sbh.ps1" -Value $sbhContent

        $output = powershell -NoProfile -File "$tmpDir\sbh.ps1" build 2>&1 | Out-String
        $output | Should Match "DISPATCH_OK:build"

        Remove-Item -Recurse -Force $tmpDir
    }
}
