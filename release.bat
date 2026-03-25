@echo off
setlocal
echo Usage: release.bat [-Version v1.3.9] [-TargetBranch dev] [-Remote origin] [-SkipPush] [-SkipRelease] [-AllowDirtyWorktree] [-DryRun]
powershell.exe -NoProfile -ExecutionPolicy Bypass -File "%~dp0release.ps1" %*
set "EXIT_CODE=%ERRORLEVEL%"
if not "%EXIT_CODE%"=="0" (
  echo.
  echo Release failed with exit code %EXIT_CODE%.
  echo See "%~dp0release.log" for diagnostics.
  echo.
  echo Press any key to close this window...
  pause >nul
)
exit /b %EXIT_CODE%
