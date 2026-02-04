# Unit tests for bin/test.ps1 (Pester 3.4.0)

$here = Split-Path -Parent $MyInvocation.MyCommand.Path
$RepoRoot = Resolve-Path "$here\..\..\.."

Describe "test.ps1" {
    It "invalid suite exits with error" {
        $output = powershell -NoProfile -Command @"
            try {
                . '$RepoRoot\bin\test.ps1' -Suite 'invalid_name'
            } catch {
                Write-Output `$_.Exception.Message
            }
            exit 1
"@ 2>&1 | Out-String

        # ValidateSet should reject invalid suite names
        $output | Should Match "valid|invalid|Cannot validate"
    }

    It "all valid suite names present in ValidateSet" {
        $content = Get-Content "$RepoRoot\bin\test.ps1" -Raw
        $content | Should Match "engine"
        $content | Should Match "host"
        $content | Should Match "readline"
        $content | Should Match "ai"
        $content | Should Match "all"
        $content | Should Match "scripts"
    }

    It "defines Test-Engine function" {
        $content = Get-Content "$RepoRoot\bin\test.ps1" -Raw
        $content | Should Match "function Test-Engine"
    }

    It "defines Test-Host function" {
        $content = Get-Content "$RepoRoot\bin\test.ps1" -Raw
        $content | Should Match "function Test-Host"
    }

    It "defines Test-Readline function" {
        $content = Get-Content "$RepoRoot\bin\test.ps1" -Raw
        $content | Should Match "function Test-Readline"
    }

    It "defines Test-Ai function" {
        $content = Get-Content "$RepoRoot\bin\test.ps1" -Raw
        $content | Should Match "function Test-Ai"
    }

    It "scripts suite invokes Pester before preflight" {
        # Verify that 'scripts' appears before Invoke-Preflight in the file
        $content = Get-Content "$RepoRoot\bin\test.ps1" -Raw
        $scriptsPos = $content.IndexOf('"scripts"')
        $preflightPos = $content.IndexOf('Invoke-Preflight')
        $scriptsPos | Should Not Be -1
        $preflightPos | Should Not Be -1
        ($scriptsPos -lt $preflightPos) | Should Be $true
    }
}
