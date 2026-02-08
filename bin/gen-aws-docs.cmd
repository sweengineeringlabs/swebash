@echo off
REM gen-aws-docs.cmd -- Windows batch wrapper for gen-aws-docs.ps1
powershell.exe -NoProfile -ExecutionPolicy Bypass -File "%~dp0gen-aws-docs.ps1" %*
