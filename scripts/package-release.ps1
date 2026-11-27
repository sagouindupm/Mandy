# VIctory S : la version téléchargeable. Compile le moteur et le lanceur, assemble dist\VIctory-S-<version>\ et
# le zippe en dist\VIctory-S-<version>.zip. Lancé aussi par .github/workflows/release.yml (tags v*).
#
#   powershell -ExecutionPolicy Bypass -File scripts\package-release.ps1
#   ... -SkipBuild                 assembler avec les exe déjà compilés (essai à blanc)
#   ... -Version 0.2.0             doit être la version de launcher/src-tauri/tauri.conf.json (le tag sans « v »)
#   ... -Screenshots docs\assets   ensuite, captures du lanceur du paquet (launcher-*.png, --preview : un dossier
#                                  personnel vide, aucune image du jeu)
#
# Dossiers de compilation : -EngineTarget (défaut : CARGO_TARGET_DIR, sinon target\) et -LauncherTarget (défaut :
# launcher\src-tauri\target). Les deux exe sont compilés pour tous les processeurs x86-64 avec AVX2
# (x86-64-v3 ; le moteur choisit aussi ses chemins AVX2 à l'exécution) et sans dépendre du runtime Visual C++
# (crt-static) : .cargo/config.toml compile pour CE processeur (target-cpu=native), ce qui ne convient pas à un
# paquet distribué.
#
# Le paquet ne contient que : VIctory S.exe, engine\victory.exe, series-s\reglages.json, scripts\rt_off_hashes.txt,
# LISEZMOI.txt, LICENSE. Jamais de fichier du jeu, de DLL NVIDIA, de capture ni de cache : une vérification
# arrête le script si autre chose s'y trouve (fichier inattendu, .rpf/.dll/.pdb, plus de 50 Mo hors victory.exe).
param(
    [string]$Version = '',
    [switch]$SkipBuild,
    [string]$EngineTarget = '',
    [string]$LauncherTarget = '',
    [string]$Screenshots = ''
)
$ErrorActionPreference = 'Stop'
$Root = Split-Path -Parent $PSScriptRoot
Set-Location $Root

$conf = Get-Content (Join-Path $Root 'launcher\src-tauri\tauri.conf.json') -Raw | ConvertFrom-Json
if (-not $Version) { $Version = $conf.version }
if ($Version -ne $conf.version) {
    throw "Version $Version demandée, mais launcher/src-tauri/tauri.conf.json dit $($conf.version) : mettez-les d'accord (et launcher/src-tauri/Cargo.toml)."
}
if (-not $EngineTarget) { $EngineTarget = if ($env:CARGO_TARGET_DIR) { $env:CARGO_TARGET_DIR } else { Join-Path $Root 'target' } }
if (-not $LauncherTarget) { $LauncherTarget = Join-Path $Root 'launcher\src-tauri\target' }
$engineExe = Join-Path $EngineTarget 'release\victory.exe'
$launcherExe = Join-Path $LauncherTarget 'release\sagouindupm-launcher.exe'

function Invoke-Step($what, [scriptblock]$run) {
    Write-Host "==> $what" -ForegroundColor Magenta
    & $run
    if ($LASTEXITCODE) { throw "$what : échec (code $LASTEXITCODE)" }
}

# ------------------------------------------------------------------ compilation
if (-not $SkipBuild) {
    $savedFlags = $env:RUSTFLAGS; $savedTarget = $env:CARGO_TARGET_DIR
    try {
        $env:RUSTFLAGS = '-C target-cpu=x86-64-v3 -C target-feature=+crt-static'
        $env:CARGO_TARGET_DIR = $EngineTarget
        Invoke-Step 'Moteur : cargo build --release --locked -p victory' { cargo build --release --locked -p victory }
        $env:CARGO_TARGET_DIR = $LauncherTarget
        Push-Location (Join-Path $Root 'launcher')
        try {
            Invoke-Step 'Lanceur : npm ci' { npm ci --no-audit --no-fund }
            Invoke-Step 'Lanceur : npm run tauri build -- --no-bundle' { npm run tauri build -- --no-bundle }
        } finally { Pop-Location }
    } finally { $env:RUSTFLAGS = $savedFlags; $env:CARGO_TARGET_DIR = $savedTarget }
}
foreach ($f in $engineExe, $launcherExe) { if (-not (Test-Path $f)) { throw "Introuvable : $f (compilez d'abord, ou sans -SkipBuild)." } }

# ------------------------------------------------------------------ assemblage
$name = "VIctory-S-$Version"
$dist = Join-Path $Root 'dist'
$out = Join-Path $dist $name
$zip = Join-Path $dist "$name.zip"
Write-Host "==> Assemblage de dist\$name" -ForegroundColor Magenta
Remove-Item -Recurse -Force $out, $zip -ErrorAction SilentlyContinue
New-Item -ItemType Directory -Force (Join-Path $out 'engine'), (Join-Path $out 'series-s'), (Join-Path $out 'scripts') | Out-Null
Copy-Item $launcherExe (Join-Path $out 'VIctory S.exe')
Copy-Item $engineExe (Join-Path $out 'engine\victory.exe')
Copy-Item (Join-Path $Root 'series-s\reglages.json') (Join-Path $out 'series-s\reglages.json')
Copy-Item (Join-Path $Root 'scripts\rt_off_hashes.txt') (Join-Path $out 'scripts\rt_off_hashes.txt')
Copy-Item (Join-Path $Root 'LICENSE') (Join-Path $out 'LICENSE')
# LISEZMOI.txt pour le Bloc-notes : UTF-8 avec BOM, fins de ligne Windows.
$readme = (Get-Content (Join-Path $Root 'scripts\release\LISEZMOI.txt') -Raw -Encoding UTF8).Replace('{VERSION}', $Version) -replace "`r?`n", "`r`n"
[IO.File]::WriteAllText((Join-Path $out 'LISEZMOI.txt'), $readme, (New-Object Text.UTF8Encoding $true))

# ------------------------------------------------------------------ vérification : rien d'autre que la liste
$expected = @('VIctory S.exe', 'engine\victory.exe', 'series-s\reglages.json', 'scripts\rt_off_hashes.txt', 'LISEZMOI.txt', 'LICENSE')
function Test-Package($entries) {
    # $entries : chemins relatifs (séparateur \) et tailles.
    $problems = @()
    foreach ($e in $entries) {
        $rel = $e.Path
        if ($expected -notcontains $rel) { $problems += "fichier inattendu : $rel" }
        if ($rel -match '\.(rpf|dll|pdb|bin|dat|dds|xvd|xvc|msixvc|awc|bik|png|jpg|jpeg|log|toml)$') { $problems += "type interdit dans le paquet : $rel" }
        if ($e.Size -gt 50MB -and $rel -ne 'engine\victory.exe') { $problems += "plus de 50 Mo : $rel ($([Math]::Round($e.Size / 1MB)) Mo)" }
        if ($rel -match '(^|\\)(sl\.|nvngx)') { $problems += "DLL NVIDIA : $rel" }
    }
    foreach ($x in $expected) { if (-not ($entries | Where-Object { $_.Path -eq $x })) { $problems += "manquant : $x" } }
    if ($problems) { $problems | ForEach-Object { Write-Host "  $_" -ForegroundColor Red }; throw 'Paquet refusé.' }
}
$files = Get-ChildItem -Recurse -File $out | ForEach-Object {
    [pscustomobject]@{ Path = $_.FullName.Substring($out.Length + 1); Size = $_.Length }
}
Test-Package $files

# ------------------------------------------------------------------ zip, puis la même vérification sur le zip
Write-Host "==> $name.zip" -ForegroundColor Magenta
Add-Type -AssemblyName System.IO.Compression.FileSystem
[IO.Compression.ZipFile]::CreateFromDirectory($out, $zip, [IO.Compression.CompressionLevel]::Optimal, $true)
$z = [IO.Compression.ZipFile]::OpenRead($zip)
try {
    $inZip = $z.Entries | Where-Object { $_.Name } | ForEach-Object {
        [pscustomobject]@{ Path = ($_.FullName -replace '/', '\').Substring($name.Length + 1); Size = $_.Length; Packed = $_.CompressedLength }
    }
} finally { $z.Dispose() }
Test-Package $inZip

$zipSize = (Get-Item $zip).Length
Write-Host ''
Write-Host "Paquet : $zip ($([Math]::Round($zipSize / 1MB, 1)) Mo)" -ForegroundColor Green
$inZip | ForEach-Object { '  {0,-28} {1,8:N0} Ko' -f $_.Path, ($_.Size / 1KB) }

# ------------------------------------------------------------------ captures du lanceur (README)
if ($Screenshots) {
    $shots = if ([IO.Path]::IsPathRooted($Screenshots)) { $Screenshots } else { Join-Path $Root $Screenshots }
    Write-Host "==> Captures du lanceur dans $shots (--preview)" -ForegroundColor Magenta
    $p = Start-Process -FilePath (Join-Path $out 'VIctory S.exe') -ArgumentList '--preview', "`"$shots`"" -WorkingDirectory $out -PassThru
    if (-not $p.WaitForExit(120000)) { Stop-Process -Id $p.Id; throw 'Le lanceur ne s''est pas fermé : captures incomplètes.' }
    Get-ChildItem $shots -Filter 'launcher-*.png' | ForEach-Object { '  {0,-28} {1,8:N0} Ko' -f $_.Name, ($_.Length / 1KB) }
}
