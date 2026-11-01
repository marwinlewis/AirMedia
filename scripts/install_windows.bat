@echo off
setlocal
title AirMedia Installer
echo ==========================================================
echo        AirMedia - Desktop Installation and Setup
echo ==========================================================
powershell -NoProfile -ExecutionPolicy Bypass -File "%~dp0install_windows.ps1" %*
if %ERRORLEVEL% neq 0 (
    echo.
    echo Installation failed with exit code %ERRORLEVEL%.
    pause
    exit /b %ERRORLEVEL%
)
endlocal
