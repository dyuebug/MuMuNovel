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
echo Usage: redeploy.bat [-NoCache] [-UseCnMirror] [-SkipFrontendBuild] [-SkipAssetVerification] [-FullRestart] [other redeploy.ps1 args]
powershell.exe -NoProfile -ExecutionPolicy Bypass -File "%~dp0redeploy.ps1" %PS_ARGS%
set "EXIT_CODE=%ERRORLEVEL%"
if not "%EXIT_CODE%"=="0" (
  echo.
  echo Redeploy failed with exit code %EXIT_CODE%.
  echo See "%~dp0logs\ops\redeploy.log" for diagnostics.
  if not defined NO_PAUSE_FLAG (
    echo.
    echo Press any key to close this window...
    pause >nul
  )
)
exit /b %EXIT_CODE%
