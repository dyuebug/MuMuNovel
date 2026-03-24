@echo off
setlocal
echo Usage: redeploy.bat [-NoCache] [-UseCnMirror] [-SkipFrontendBuild] [-SkipAssetVerification] [-FullRestart] [other redeploy.ps1 args]
powershell.exe -NoProfile -ExecutionPolicy Bypass -File "%~dp0redeploy.ps1" %*
set "EXIT_CODE=%ERRORLEVEL%"
if not "%EXIT_CODE%"=="0" (
  echo.
  echo Redeploy failed with exit code %EXIT_CODE%.
  echo See "%~dp0redeploy.log" for diagnostics.
  echo.
  echo Press any key to close this window...
  pause >nul
)
exit /b %EXIT_CODE%
