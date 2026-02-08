# E2E tests for gen-aws-docs.ps1 (Pester 3.4.0)

$here = Split-Path -Parent $MyInvocation.MyCommand.Path
$RepoRoot = Resolve-Path "$here\..\..\.."
$Script = Join-Path $RepoRoot "bin\gen-aws-docs.ps1"
$Sbh = Join-Path $RepoRoot "sbh.ps1"

# -- Helper: Create mock aws CLI --------------------------------------
function New-AwsMock {
    $mockDir = Join-Path $env:TEMP "pester-aws-mock-$(Get-Random)"
    New-Item -ItemType Directory -Path $mockDir -Force | Out-Null

    # Create aws.cmd (Windows batch file mock)
    $mockScript = @'
@echo off
if "%1"=="--version" (
    echo aws-cli/2.0.0-mock Python/3.9.0 Windows/10 source/AMD64
    exit /b 0
)
if "%2"=="help" (
    echo %1()                                                                %1()
    echo.
    echo NAME
    echo        %1 -
    echo.
    echo DESCRIPTION
    echo        Amazon %1 service provides functionality.
    echo.
    echo SYNOPSIS
    echo           aws %1 ^<command^> [^<args^>...]
    echo.
    echo AVAILABLE COMMANDS
    echo        o create-%1-resource
    echo.
    echo        o delete-%1-resource
    echo.
    echo        o describe-%1-resources
    echo.
    echo        o help
    echo.
    echo OPTIONS
    echo        None
    exit /b 0
)
echo Unknown command: %* 1>&2
exit /b 1
'@
    Set-Content -Path "$mockDir\aws.cmd" -Value $mockScript

    # Also create aws.exe wrapper (PowerShell script renamed)
    $psWrapper = @'
param($Args)
$allArgs = $Args -join ' '
& cmd /c "$PSScriptRoot\aws.cmd" $Args
'@
    # Actually, let's just use the .cmd file since Windows will find it

    return $mockDir
}

# =====================================================================
# Help and usage tests
# =====================================================================

Describe "gen-aws-docs.ps1 help and usage" {
    It "-Help shows usage and exits 0" {
        $output = powershell -NoProfile -ExecutionPolicy Bypass -File $Script -Help 2>&1 | Out-String
        $LASTEXITCODE | Should Be 0
        $output | Should Match "Usage"
    }

    It "help shows options" {
        $output = powershell -NoProfile -ExecutionPolicy Bypass -File $Script -Help 2>&1 | Out-String
        $output | Should Match "-Install"
        $output | Should Match "-y"
    }

    It "help shows environment variables" {
        $output = powershell -NoProfile -ExecutionPolicy Bypass -File $Script -Help 2>&1 | Out-String
        $output | Should Match "SWEBASH_AWS_INSTALL"
        $output | Should Match "SWEBASH_AWS_DOCS_DIR"
    }

    It "help shows configurable URL env var" {
        $output = powershell -NoProfile -ExecutionPolicy Bypass -File $Script -Help 2>&1 | Out-String
        $output | Should Match "SWEBASH_AWS_CLI_URL_WINDOWS"
    }
}

# =====================================================================
# Missing AWS CLI behavior
# =====================================================================

Describe "gen-aws-docs.ps1 without AWS CLI" {
    It "exits with error when AWS CLI not found" {
        # Run with empty PATH so aws isn't found
        $result = powershell -NoProfile -ExecutionPolicy Bypass -Command @"
            `$env:Path = 'C:\Windows\System32'
            & '$Script' 2>&1
            exit `$LASTEXITCODE
"@ 2>&1 | Out-String
        $LASTEXITCODE | Should Be 1
    }

    It "suggests install URL when AWS CLI not found" {
        $result = powershell -NoProfile -ExecutionPolicy Bypass -Command @"
            `$env:Path = 'C:\Windows\System32'
            & '$Script' 2>&1
"@ 2>&1 | Out-String
        $result | Should Match "docs.aws.amazon.com"
    }

    It "mentions -Install flag when AWS CLI not found" {
        $result = powershell -NoProfile -ExecutionPolicy Bypass -Command @"
            `$env:Path = 'C:\Windows\System32'
            & '$Script' 2>&1
"@ 2>&1 | Out-String
        $result | Should Match "-Install"
    }
}

# =====================================================================
# sbh.ps1 integration
# =====================================================================

Describe "sbh.ps1 gen-aws-docs integration" {
    It "sbh.ps1 help lists gen-aws-docs command" {
        $output = powershell -NoProfile -ExecutionPolicy Bypass -File $Sbh --help 2>&1 | Out-String
        $output | Should Match "gen-aws-docs"
    }

    It "sbh.ps1 help shows -Install option" {
        $output = powershell -NoProfile -ExecutionPolicy Bypass -File $Sbh --help 2>&1 | Out-String
        $output | Should Match "-Install"
    }

    It "sbh.ps1 dispatches gen-aws-docs to correct script" {
        # Should fail (no aws) but should route to the right script
        $result = powershell -NoProfile -ExecutionPolicy Bypass -Command @"
            `$env:Path = 'C:\Windows\System32'
            & '$Sbh' 'gen-aws-docs' 2>&1
"@ 2>&1 | Out-String
        # Should NOT show sbh usage, should show gen-aws-docs output
        $result | Should Not Match "Commands:"
        $result | Should Match "AWS"
    }
}

# =====================================================================
# URL configuration tests
# =====================================================================

Describe "gen-aws-docs.ps1 configurable URLs" {
    It "uses default AWS URL when env var not set" {
        $result = powershell -NoProfile -ExecutionPolicy Bypass -Command @"
            `$env:SWEBASH_AWS_CLI_URL_WINDOWS = `$null
            . '$Script'
            Write-Output `$AwsCliUrl
"@ 2>&1 | Out-String
        $result.Trim() | Should Match "awscli.amazonaws.com"
        $result.Trim() | Should Match "\.msi"
    }

    It "uses custom URL from SWEBASH_AWS_CLI_URL_WINDOWS env var" {
        $customUrl = "https://mirror.example.com/custom-aws.msi"
        $result = powershell -NoProfile -ExecutionPolicy Bypass -Command @"
            `$env:SWEBASH_AWS_CLI_URL_WINDOWS = '$customUrl'
            . '$Script'
            Write-Output `$AwsCliUrl
"@ 2>&1 | Out-String
        $result.Trim() | Should Be $customUrl
    }
}

# =====================================================================
# Output directory configuration
# =====================================================================

Describe "gen-aws-docs.ps1 output directory" {
    It "uses default output directory when env var not set" {
        $result = powershell -NoProfile -ExecutionPolicy Bypass -Command @"
            `$env:SWEBASH_AWS_DOCS_DIR = `$null
            . '$Script'
            Write-Output `$OutputDir
"@ 2>&1 | Out-String
        $result.Trim() | Should Match "\.config.swebash.docs.aws"
    }

    It "uses custom output directory from SWEBASH_AWS_DOCS_DIR env var" {
        $customDir = "C:\custom\docs\path"
        $result = powershell -NoProfile -ExecutionPolicy Bypass -Command @"
            `$env:SWEBASH_AWS_DOCS_DIR = '$customDir'
            . '$Script'
            Write-Output `$OutputDir
"@ 2>&1 | Out-String
        $result.Trim() | Should Be $customDir
    }
}

# =====================================================================
# Install flag behavior
# =====================================================================

Describe "gen-aws-docs.ps1 install flag" {
    It "SWEBASH_AWS_INSTALL=yes triggers auto-install mode" {
        $result = powershell -NoProfile -ExecutionPolicy Bypass -Command @"
            `$env:SWEBASH_AWS_INSTALL = 'yes'
            `$env:Path = 'C:\Windows\System32'
            & '$Script' 2>&1
"@ 2>&1 | Out-String
        # Should attempt to auto-install (will fail without network, but should try)
        $result | Should Match "Auto-installing|Installing"
    }
}

# =====================================================================
# Helper function tests (via dot-sourcing)
# =====================================================================

Describe "gen-aws-docs.ps1 helper functions" {
    It "Test-Command returns true for existing command" {
        $result = powershell -NoProfile -ExecutionPolicy Bypass -Command @"
            . '$Script'
            if (Test-Command 'powershell') { 'exists' } else { 'missing' }
"@ 2>&1 | Out-String
        $result.Trim() | Should Be "exists"
    }

    It "Test-Command returns false for missing command" {
        $result = powershell -NoProfile -ExecutionPolicy Bypass -Command @"
            . '$Script'
            if (Test-Command 'nonexistent_cmd_xyz_123') { 'exists' } else { 'missing' }
"@ 2>&1 | Out-String
        $result.Trim() | Should Be "missing"
    }

    It "Remove-Formatting strips ANSI escape codes" {
        # Build the ANSI string with escape chars
        $esc = [char]27
        $testInput = "${esc}[1mBOLD${esc}[0m normal"
        $result = powershell -NoProfile -ExecutionPolicy Bypass -Command "
            . '$Script'
            Remove-Formatting '$testInput'
        " 2>&1 | Out-String
        $result.Trim() | Should Be "BOLD normal"
    }

    It "Remove-Formatting strips carriage returns" {
        $result = powershell -NoProfile -ExecutionPolicy Bypass -Command @'
            . $args[0]
            Remove-Formatting "line one`r`nline two"
'@ -args $Script 2>&1 | Out-String
        $result | Should Match "line one"
        $result | Should Match "line two"
    }
}

# =====================================================================
# Services list configuration
# =====================================================================

Describe "gen-aws-docs.ps1 services configuration" {
    It "has expected core services in list" {
        $result = powershell -NoProfile -ExecutionPolicy Bypass -Command @"
            . '$Script'
            `$Services -join ','
"@ 2>&1 | Out-String
        $result | Should Match "ec2"
        $result | Should Match "s3"
        $result | Should Match "lambda"
        $result | Should Match "iam"
    }

    It "has expected number of services" {
        $result = powershell -NoProfile -ExecutionPolicy Bypass -Command @"
            . '$Script'
            `$Services.Count
"@ 2>&1 | Out-String
        [int]$result.Trim() | Should BeGreaterThan 10
    }
}
