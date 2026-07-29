@echo off
setlocal EnableExtensions
chcp 65001 >nul
set "NO_PAUSE_FLAG="
if defined CI set "NO_PAUSE_FLAG=1"
if not exist CONIN$ set "NO_PAUSE_FLAG=1"
if not exist CONOUT$ set "NO_PAUSE_FLAG=1"
for %%A in (%*) do (
  if /I "%%~A"=="-NoPause" set "NO_PAUSE_FLAG=1"
  if /I "%%~A"=="-NonInteractive" set "NO_PAUSE_FLAG=1"
)

echo.
echo MuMuNovel Fast Redeploy
echo =======================
echo Fast path for code-only changes: rebuild Rust backend, restart runtime, and verify.
echo.
echo Usage: redeploy-fast.bat [-BuildFrontend^|-SkipFrontendBuild] [-NoCache] [-UseCnMirror] [-SkipSmoke] [-NoPause^|-NonInteractive]
echo        Default: auto-detect changes; prompt when frontend/static files changed and console is interactive.
echo.
set "PS_HOST_FLAGS="
if defined NO_PAUSE_FLAG set "PS_HOST_FLAGS=-NonInteractive"
powershell.exe %PS_HOST_FLAGS% -NoProfile -ExecutionPolicy Bypass -File "%~dp0redeploy-fast.ps1" %*
set "EXIT_CODE=%ERRORLEVEL%"
if not "%EXIT_CODE%"=="0" (
  echo.
  echo Fast redeploy failed with exit code %EXIT_CODE%.
  echo See "%~dp0logs\ops\redeploy-fast.log" for diagnostics.
  if not defined NO_PAUSE_FLAG (
    echo.
    echo Press any key to close this window...
    pause >nul
  )
)
exit /b %EXIT_CODE%
