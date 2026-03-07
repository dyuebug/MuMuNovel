@echo off
setlocal
echo Usage: redeploy.bat [-NoCache] [-SkipAssetVerification] [other redeploy.ps1 args]
powershell.exe -NoProfile -ExecutionPolicy Bypass -File "%~dp0redeploy.ps1" %*
set "EXIT_CODE=%ERRORLEVEL%"
if not "%EXIT_CODE%"=="0" (
  echo.
  echo Redeploy failed with exit code %EXIT_CODE%.
)
exit /b %EXIT_CODE%
