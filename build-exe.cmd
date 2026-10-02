@echo off
powershell -NoProfile -ExecutionPolicy Bypass -File "%~dp0scripts\build-app.ps1" -Mode exe -StopRunning
pause
