@echo off
setlocal
echo.
echo MuMuNovel Strangler Fig Deploy
echo ==============================
echo Python + Rust + Nginx (3-service architecture)
echo.
echo Usage: deploy-strangler.bat [-NoCache] [-UseCnMirror] [-SkipFrontendBuild] [-FullRestart] [-NoPause] [-RepairPostgresPassword]
echo.
powershell.exe -NoProfile -ExecutionPolicy Bypass -File "%~dp0deploy-strangler.ps1" %*
set "EXIT_CODE=%ERRORLEVEL%"
if not "%EXIT_CODE%"=="0" (
  echo.
  echo Strangler deploy failed with exit code %EXIT_CODE%.
  echo See "%~dp0logs\ops\deploy-strangler.log" for diagnostics.
  echo.
  echo Press any key to close this window...
  pause >nul
)
exit /b %EXIT_CODE%
