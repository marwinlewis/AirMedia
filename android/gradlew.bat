@rem Gradle Wrapper Batch Script for AirMedia
@echo off
set DIRNAME=%~dp0

where gradle >nul 2>nul
if %ERRORLEVEL% equ 0 (
    gradle %*
    exit /b %ERRORLEVEL%
)

set WRAPPER_JAR=%DIRNAME%gradle\wrapper\gradle-wrapper.jar
if not exist "%WRAPPER_JAR%" (
    echo Downloading Gradle wrapper jar...
    powershell -Command "[Net.ServicePointManager]::SecurityProtocol = [Net.SecurityProtocolType]::Tls12; (New-Object Net.WebClient).DownloadFile('https://raw.githubusercontent.com/gradle/gradle/v8.7.0/gradle/wrapper/gradle-wrapper.jar', '%WRAPPER_JAR%')" 2>nul
)

if exist "%WRAPPER_JAR%" (
    java -jar "%WRAPPER_JAR%" %*
    exit /b %ERRORLEVEL%
)

echo ERROR: Please install Gradle or Java to run gradle wrapper.
exit /b 1
