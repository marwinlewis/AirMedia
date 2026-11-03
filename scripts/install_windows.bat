@echo off
setlocal
title AirMedia Installer
echo ==========================================================
echo        AirMedia - Desktop Installation and Setup
echo ==========================================================

REM Check if native Inno Setup installer exists
REM Only match AirMedia's own installer, never other *setup*.exe files that
REM happen to sit next to the extracted folder (e.g. in Downloads).
for %%F in ("%~dp0airmedia-*-setup.exe" "%~dp0..\airmedia-*-setup.exe") do (
    if exist "%%~fF" (
        echo Launching native Windows installer: %%~nxF
        start "" "%%~fF"
        exit /b 0
    )
)

REM Fallback to clean PowerShell install
powershell -NoProfile -ExecutionPolicy RemoteSigned -File "%~dp0install.ps1" %*
if %ERRORLEVEL% neq 0 (
    echo.
    echo Installation completed with status code %ERRORLEVEL%.
    pause
    exit /b %ERRORLEVEL%
)
endlocal
