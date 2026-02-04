# sbh.ps1 -- swebash quickstart entrypoint (Windows)
$RepoRoot = $PSScriptRoot

function Show-Usage {
    Write-Host "Usage: .\sbh <command> [options]"
    Write-Host ""
    Write-Host "Commands:"
    Write-Host "  setup          One-time environment setup"
    Write-Host "  build          Build engine (WASM) and host"
    Write-Host "    -Debug         Build in debug mode (default: release)"
    Write-Host "  run            Build if needed and launch swebash"
    Write-Host "    -Release       Run release build"
    Write-Host "    -Debug         Run debug build (default)"
    Write-Host "  test [suite]   Build and run tests"
    Write-Host "    engine|host|readline|ai|all (default: all)"
}

$Command = if ($args.Count -gt 0) { $args[0] } else { "" }
$Rest = @($args | Select-Object -Skip 1)

switch ($Command) {
    "setup" { & "$RepoRoot\bin\setup.ps1" @Rest }
    "build" { & "$RepoRoot\bin\build.ps1" @Rest }
    "run"   { & "$RepoRoot\bin\run.ps1"   @Rest }
    "test"  { & "$RepoRoot\bin\test.ps1"  @Rest }
    { $_ -in "help", "-h", "--help", "" } { Show-Usage }
    default {
        Show-Usage
        exit 1
    }
}
