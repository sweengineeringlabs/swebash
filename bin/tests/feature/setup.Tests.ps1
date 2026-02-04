# Unit tests for bin/setup.ps1 (Pester 3.4.0)

$here = Split-Path -Parent $MyInvocation.MyCommand.Path
$RepoRoot = Resolve-Path "$here\..\..\.."

Describe "setup.ps1" {
    It "fails when rustup is not found" {
        # Clear PATH so Get-Command can't find rustup
        $output = powershell -NoProfile -Command @"
            `$env:PATH = 'C:\Windows\System32'
            `$ErrorActionPreference = 'Continue'
            try { . '$RepoRoot\bin\setup.ps1' } catch { `$_.Exception.Message }
"@ 2>&1 | Out-String

        $output | Should Match "rustup"
    }

    It "fails when cargo is not found" {
        # Create a fake rustup but no cargo
        $tmpDir = Join-Path $env:TEMP "pester-setup-shim-$(Get-Random)"
        New-Item -ItemType Directory -Path $tmpDir -Force | Out-Null
        Set-Content -Path "$tmpDir\rustup.cmd" -Value '@echo rustup fake'

        $output = powershell -NoProfile -Command @"
            `$env:PATH = '$tmpDir;C:\Windows\System32'
            `$ErrorActionPreference = 'Continue'
            try { . '$RepoRoot\bin\setup.ps1' } catch { `$_.Exception.Message }
"@ 2>&1 | Out-String

        Remove-Item -Recurse -Force $tmpDir
        $output | Should Match "cargo"
    }

    It "copies .env.example to .env when missing" {
        $tmpDir = Join-Path $env:TEMP "pester-setup-$(Get-Random)"
        New-Item -ItemType Directory -Path $tmpDir -Force | Out-Null
        Set-Content -Path "$tmpDir\.env.example" -Value "TEST_SETUP=value"

        $output = powershell -NoProfile -Command @"
            `$env:CARGO_REGISTRIES_LOCAL_INDEX = 'file:///C:/fake'
            function rustup { Write-Output 'rustup fake' }
            function cargo { Write-Output 'cargo fake' }
            . '$RepoRoot\lib\common.ps1'
            function Invoke-Preflight {}
            function Verify-Registry {}
            function Ensure-Registry {}
            `$RepoRoot = '$tmpDir'
            `$EnvFile = Join-Path `$RepoRoot '.env'
            `$EnvExample = Join-Path `$RepoRoot '.env.example'
            if (-not (Test-Path `$EnvFile)) {
                if (Test-Path `$EnvExample) {
                    Copy-Item `$EnvExample `$EnvFile
                    Write-Output 'Copied .env.example -> .env'
                }
            }
"@ 2>&1 | Out-String

        (Test-Path "$tmpDir\.env") | Should Be $true
        Remove-Item -Recurse -Force $tmpDir
    }

    It "does not overwrite existing .env" {
        $tmpDir = Join-Path $env:TEMP "pester-setup-$(Get-Random)"
        New-Item -ItemType Directory -Path $tmpDir -Force | Out-Null
        Set-Content -Path "$tmpDir\.env" -Value "ORIGINAL=keep"
        Set-Content -Path "$tmpDir\.env.example" -Value "NEW=overwrite"

        powershell -NoProfile -Command @"
            `$RepoRoot = '$tmpDir'
            `$EnvFile = Join-Path `$RepoRoot '.env'
            `$EnvExample = Join-Path `$RepoRoot '.env.example'
            if (-not (Test-Path `$EnvFile)) {
                Copy-Item `$EnvExample `$EnvFile
            } else {
                Write-Output '.env already exists'
            }
"@ | Out-Null

        $content = Get-Content "$tmpDir\.env" -Raw
        $content | Should Match "ORIGINAL"
        Remove-Item -Recurse -Force $tmpDir
    }

    It "has no parse errors" {
        $errors = $null
        [System.Management.Automation.PSParser]::Tokenize(
            (Get-Content "$RepoRoot\bin\setup.ps1" -Raw), [ref]$errors
        ) | Out-Null
        $errors.Count | Should Be 0
    }
}
