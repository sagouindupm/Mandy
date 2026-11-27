@echo off
rem Double-cliquez ici : lanceur graphique de Sagouindupm ^| VIctory, pour debutant.
rem Le lanceur console (utilisateurs avances) reste Sagouindupm.bat.
rem Premier lancement : construit la fenetre du lanceur (launcher\, Tauri) avec Node.js et Rust,
rem en proposant de les installer avec winget s'ils manquent. Ensuite, demarre directement.
setlocal EnableExtensions
for %%I in ("%~dp0..") do set "ROOT=%%~fI"
set "APP=%ROOT%\launcher"
set "SAGOUINDUPM_ROOT=%ROOT%"
if defined CARGO_TARGET_DIR (set "EXE=%CARGO_TARGET_DIR%\release\sagouindupm-launcher.exe") else (set "EXE=%APP%\src-tauri\target\release\sagouindupm-launcher.exe")
if exist "%EXE%" goto run

echo.
echo  Sagouindupm ^| VIctory : premier lancement.
echo  Construction de la fenetre du lanceur (5 a 10 minutes, une seule fois).
echo.
if exist "%USERPROFILE%\.cargo\bin\cargo.exe" set "PATH=%USERPROFILE%\.cargo\bin;%PATH%"
if exist "%ProgramFiles%\nodejs\npm.cmd" set "PATH=%ProgramFiles%\nodejs;%PATH%"

where npm >nul 2>nul
if errorlevel 1 (
  echo  Node.js est introuvable : il sert a construire la fenetre du lanceur.
  call :install OpenJS.NodeJS.LTS "Node.js" || goto fail
  set "PATH=%ProgramFiles%\nodejs;%PATH%"
)
where cargo >nul 2>nul
if errorlevel 1 (
  echo  Rust est introuvable : il sert a compiler le lanceur et VIctory.
  call :install Rustlang.Rustup "Rust" || goto fail
  set "PATH=%USERPROFILE%\.cargo\bin;%PATH%"
)
set "VSWHERE=%ProgramFiles(x86)%\Microsoft Visual Studio\Installer\vswhere.exe"
set "VC="
if exist "%VSWHERE%" for /f "usebackq delims=" %%V in (`"%VSWHERE%" -products * -latest -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -property installationPath`) do set "VC=%%V"
if not defined VC (
  echo  Les outils C++ de Microsoft sont introuvables : Rust en a besoin sous Windows ^(plusieurs Go^).
  call :install Microsoft.VisualStudio.2022.BuildTools "les outils C++ de Microsoft" "--override" "--quiet --wait --add Microsoft.VisualStudio.Workload.VCTools --includeRecommended" || goto fail
)

pushd "%APP%" || goto fail
echo  [1/2] Paquets npm...
call npm ci --no-audit --no-fund
if errorlevel 1 (popd & goto fail)
echo  [2/2] Compilation de la fenetre...
call npm run tauri build -- --no-bundle
if errorlevel 1 (popd & goto fail)
popd
if not exist "%EXE%" goto fail

:run
start "" "%EXE%" --root "%ROOT%"
exit /b 0

:install
rem %1 : identifiant winget, %2 : nom affiche, %3 %4 : options en plus.
where winget >nul 2>nul
if errorlevel 1 (
  echo  winget est absent : installez %~2 vous-meme, puis relancez ce fichier.
  exit /b 1
)
choice /c ON /m " Installer %~2 maintenant avec winget"
if errorlevel 2 exit /b 1
if "%~3"=="" (
  winget install --id %1 -e --accept-package-agreements --accept-source-agreements
) else (
  winget install --id %1 -e --accept-package-agreements --accept-source-agreements %3 %4
)
if errorlevel 1 (
  echo  L'installation de %~2 a echoue.
  exit /b 1
)
exit /b 0

:fail
echo.
echo  Le lanceur n'a pas pu etre construit : voir les messages ci-dessus.
echo  Il faut Node.js, Rust et les outils C++ de Microsoft. Le lanceur console reste disponible : Sagouindupm.bat
echo.
pause
exit /b 1
