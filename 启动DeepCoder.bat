@echo off
set "SCRIPT_DIR=%~dp0"
powershell.exe -NoLogo -NoProfile -ExecutionPolicy Bypass -File "%SCRIPT_DIR%StartDeepCoder.ps1"
if errorlevel 1 (
  echo.
  echo DeepCoder launch failed. Please check the error above.
  pause
)
