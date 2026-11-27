@echo off
rem Double-click here: graphical launcher of Sagouindupm ^| VIctory, for beginners.
rem The console launcher (advanced users) is still Sagouindupm.bat.
rem First launch: builds the launcher window (launcher\, Tauri) with Node.js and Rust,
rem offering to install them with winget if they are missing. After that, it starts directly.
setlocal EnableExtensions
for %%I in ("%~dp0..") do set "ROOT=%%~fI"
set "APP=%ROOT%\launcher"
set "SAGOUINDUPM_ROOT=%ROOT%"
if defined CARGO_TARGET_DIR (set "EXE=%CARGO_TARGET_DIR%\release\sagouindupm-launcher.exe") else (set "EXE=%APP%\src-tauri\target\release\sagouindupm-launcher.exe")
if exist "%EXE%" goto run

echo.
echo  Sagouindupm ^| VIctory: first launch.
echo  Building the launcher window (5 to 10 minutes, only once).
echo.
if exist "%USERPROFILE%\.cargo\bin\cargo.exe" set "PATH=%USERPROFILE%\.cargo\bin;%PATH%"
if exist "%ProgramFiles%\nodejs\npm.cmd" set "PATH=%ProgramFiles%\nodejs;%PATH%"

where npm >nul 2>nul
if errorlevel 1 (
  echo  Node.js not found: it is needed to build the launcher window.
  call :install OpenJS.NodeJS.LTS "Node.js" || goto fail
  set "PATH=%ProgramFiles%\nodejs;%PATH%"
)
where cargo >nul 2>nul
if errorlevel 1 (
  echo  Rust not found: it is needed to compile the launcher and VIctory.
  call :install Rustlang.Rustup "Rust" || goto fail
  set "PATH=%USERPROFILE%\.cargo\bin;%PATH%"
)
set "VSWHERE=%ProgramFiles(x86)%\Microsoft Visual Studio\Installer\vswhere.exe"
set "VC="
if exist "%VSWHERE%" for /f "usebackq delims=" %%V in (`"%VSWHERE%" -products * -latest -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -property installationPath`) do set "VC=%%V"
if not defined VC (
  echo  Microsoft C++ tools not found: Rust needs them on Windows ^(several GB^).
  call :install Microsoft.VisualStudio.2022.BuildTools "the Microsoft C++ tools" "--override" "--quiet --wait --add Microsoft.VisualStudio.Workload.VCTools --includeRecommended" || goto fail
)

pushd "%APP%" || goto fail
echo  [1/2] npm packages...
call npm ci --no-audit --no-fund
if errorlevel 1 (popd & goto fail)
echo  [2/2] Compiling the window...
call npm run tauri build -- --no-bundle
if errorlevel 1 (popd & goto fail)
popd
if not exist "%EXE%" goto fail

:run
start "" "%EXE%" --root "%ROOT%"
exit /b 0

:install
rem %1: winget id, %2: displayed name, %3 %4: extra options.
where winget >nul 2>nul
if errorlevel 1 (
  echo  winget is missing: install %~2 yourself, then run this file again.
  exit /b 1
)
choice /c YN /m " Install %~2 now with winget"
if errorlevel 2 exit /b 1
if "%~3"=="" (
  winget install --id %1 -e --accept-package-agreements --accept-source-agreements
) else (
  winget install --id %1 -e --accept-package-agreements --accept-source-agreements %3 %4
)
if errorlevel 1 (
  echo  Installing %~2 failed.
  exit /b 1
)
exit /b 0

:fail
echo.
echo  The launcher could not be built: see the messages above.
echo  It needs Node.js, Rust and the Microsoft C++ tools. The console launcher is still available: Sagouindupm.bat
echo.
pause
exit /b 1
