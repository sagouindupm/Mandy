@echo off
rem Double-cliquez ici : lanceur graphique de Sagouindupm ^| VIctory, pour debutant.
rem Le lanceur console (utilisateurs avances) reste Sagouindupm.bat.
start "" powershell -NoProfile -ExecutionPolicy Bypass -WindowStyle Hidden -STA -File "%~dp0Sagouindupm-Launcher.ps1"
