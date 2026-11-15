@echo off
powershell.exe -NoProfile -ExecutionPolicy Bypass -File "%~dp0scripts\bump_version.ps1" %*
