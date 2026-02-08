@echo off
REM sbh.cmd -- Windows batch wrapper for sbh.ps1
REM Automatically sets ExecutionPolicy Bypass so scripts run without policy issues

powershell.exe -NoProfile -ExecutionPolicy Bypass -File "%~dp0sbh.ps1" %*
