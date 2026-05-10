@echo off
setlocal EnableExtensions EnableDelayedExpansion

set "NO_PAUSE_FLAG="
if defined CI set "NO_PAUSE_FLAG=1"
for %%A in (%*) do (
    if /I "%%~A"=="-NoPause" set "NO_PAUSE_FLAG=1"
    if /I "%%~A"=="-NonInteractive" set "NO_PAUSE_FLAG=1"
)
set "PS_ARGS="
for %%A in (%*) do (
    if /I not "%%~A"=="-NoPause" if /I not "%%~A"=="-NonInteractive" set "PS_ARGS=!PS_ARGS! %%~A"
)

powershell.exe -NoProfile -ExecutionPolicy Bypass -File "%~dp0check-alembic-health.ps1" %PS_ARGS%
set "EXIT_CODE=%ERRORLEVEL%"

if not "%EXIT_CODE%"=="0" (
    echo.
    echo Alembic revision health check failed with exit code %EXIT_CODE%.
    if not defined NO_PAUSE_FLAG pause
    exit /b %EXIT_CODE%
)

exit /b 0
