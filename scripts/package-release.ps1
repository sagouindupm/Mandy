# VIctory S: the downloadable version. Builds the engine and the launcher, assembles dist\VIctory-S-<version>\ and
# zips it into dist\VIctory-S-<version>.zip. Also run by .github/workflows/release.yml (tags v*).
#
#   powershell -ExecutionPolicy Bypass -File scripts\package-release.ps1
#   ... -SkipBuild                 assemble with the exes already built (dry run)
#   ... -Version 0.2.0             must be the version of launcher/src-tauri/tauri.conf.json (the tag without "v")
#   ... -Screenshots docs\assets   afterwards, screenshots of the packaged launcher (launcher-*.png, --preview: an
#                                  empty personal folder, no game image)
#
# Build folders: -EngineTarget (default: CARGO_TARGET_DIR, otherwise target\) and -LauncherTarget (default:
# launcher\src-tauri\target). Both exes are built for every x86-64 CPU with AVX2 (x86-64-v3; the engine also picks
# its AVX2 paths at run time) and without depending on the Visual C++ runtime (crt-static): .cargo/config.toml
# builds for THIS CPU (target-cpu=native), which does not suit a distributed package.
#
# The package contains only: VIctory S.exe, engine\victory.exe, series-s\reglages.json,
# series-s\pipelines\series-s-rtx.txt, scripts\rt_off_hashes.txt, README.txt, LICENSE. Never a game file, an NVIDIA
# DLL, a screenshot or a cache: a check stops the script if anything else is there (unexpected file, .rpf/.dll/.pdb,
# more than 50 MB apart from victory.exe).
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
    throw "Version $Version requested, but launcher/src-tauri/tauri.conf.json says $($conf.version): make them match (and launcher/src-tauri/Cargo.toml)."
}
if (-not $EngineTarget) { $EngineTarget = if ($env:CARGO_TARGET_DIR) { $env:CARGO_TARGET_DIR } else { Join-Path $Root 'target' } }
if (-not $LauncherTarget) { $LauncherTarget = Join-Path $Root 'launcher\src-tauri\target' }
$engineExe = Join-Path $EngineTarget 'release\victory.exe'
$launcherExe = Join-Path $LauncherTarget 'release\sagouindupm-launcher.exe'

function Invoke-Step($what, [scriptblock]$run) {
    Write-Host "==> $what" -ForegroundColor Magenta
    & $run
    if ($LASTEXITCODE) { throw "${what}: failed (code $LASTEXITCODE)" }
}

# ------------------------------------------------------------------ build
if (-not $SkipBuild) {
    $savedFlags = $env:RUSTFLAGS; $savedTarget = $env:CARGO_TARGET_DIR
    try {
        $env:RUSTFLAGS = '-C target-cpu=x86-64-v3 -C target-feature=+crt-static'
        $env:CARGO_TARGET_DIR = $EngineTarget
        Invoke-Step 'Engine: cargo build --release --locked -p victory' { cargo build --release --locked -p victory }
        $env:CARGO_TARGET_DIR = $LauncherTarget
        Push-Location (Join-Path $Root 'launcher')
        try {
            Invoke-Step 'Launcher: npm ci' { npm ci --no-audit --no-fund }
            Invoke-Step 'Launcher: npm run tauri build -- --no-bundle' { npm run tauri build -- --no-bundle }
        } finally { Pop-Location }
    } finally { $env:RUSTFLAGS = $savedFlags; $env:CARGO_TARGET_DIR = $savedTarget }
}
foreach ($f in $engineExe, $launcherExe) { if (-not (Test-Path $f)) { throw "Not found: $f (build first, or run without -SkipBuild)." } }

# ------------------------------------------------------------------ assembly
$name = "VIctory-S-$Version"
$dist = Join-Path $Root 'dist'
$out = Join-Path $dist $name
$zip = Join-Path $dist "$name.zip"
Write-Host "==> Assembling dist\$name" -ForegroundColor Magenta
Remove-Item -Recurse -Force $out, $zip -ErrorAction SilentlyContinue
New-Item -ItemType Directory -Force (Join-Path $out 'engine'), (Join-Path $out 'series-s\pipelines'), (Join-Path $out 'scripts') | Out-Null
Copy-Item $launcherExe (Join-Path $out 'VIctory S.exe')
Copy-Item $engineExe (Join-Path $out 'engine\victory.exe')
Copy-Item (Join-Path $Root 'series-s\reglages.json') (Join-Path $out 'series-s\reglages.json')
# The shared pipeline list (keys, hashes, states: no game code), built before the first session.
Copy-Item (Join-Path $Root 'series-s\pipelines\series-s-rtx.txt') (Join-Path $out 'series-s\pipelines\series-s-rtx.txt')
Copy-Item (Join-Path $Root 'scripts\rt_off_hashes.txt') (Join-Path $out 'scripts\rt_off_hashes.txt')
Copy-Item (Join-Path $Root 'LICENSE') (Join-Path $out 'LICENSE')
# README.txt for Notepad: UTF-8 with BOM, Windows line endings.
$readme = (Get-Content (Join-Path $Root 'scripts\release\README.txt') -Raw -Encoding UTF8).Replace('{VERSION}', $Version) -replace "`r?`n", "`r`n"
[IO.File]::WriteAllText((Join-Path $out 'README.txt'), $readme, (New-Object Text.UTF8Encoding $true))

# ------------------------------------------------------------------ check: nothing but the list
$expected = @('VIctory S.exe', 'engine\victory.exe', 'series-s\reglages.json', 'series-s\pipelines\series-s-rtx.txt', 'scripts\rt_off_hashes.txt', 'README.txt', 'LICENSE')
function Test-Package($entries) {
    # $entries: relative paths (separator \) and sizes.
    $problems = @()
    foreach ($e in $entries) {
        $rel = $e.Path
        if ($expected -notcontains $rel) { $problems += "unexpected file: $rel" }
        if ($rel -match '\.(rpf|dll|pdb|bin|dat|dds|xvd|xvc|msixvc|awc|bik|png|jpg|jpeg|log|toml)$') { $problems += "file type not allowed in the package: $rel" }
        if ($e.Size -gt 50MB -and $rel -ne 'engine\victory.exe') { $problems += "over 50 MB: $rel ($([Math]::Round($e.Size / 1MB)) MB)" }
        if ($rel -match '(^|\\)(sl\.|nvngx)') { $problems += "NVIDIA DLL: $rel" }
    }
    foreach ($x in $expected) { if (-not ($entries | Where-Object { $_.Path -eq $x })) { $problems += "missing: $x" } }
    if ($problems) { $problems | ForEach-Object { Write-Host "  $_" -ForegroundColor Red }; throw 'Package rejected.' }
}
$files = Get-ChildItem -Recurse -File $out | ForEach-Object {
    [pscustomobject]@{ Path = $_.FullName.Substring($out.Length + 1); Size = $_.Length }
}
Test-Package $files

# ------------------------------------------------------------------ zip, then the same check on the zip
Write-Host "==> $name.zip" -ForegroundColor Magenta
Add-Type -AssemblyName System.IO.Compression, System.IO.Compression.FileSystem
# Entry by entry, with "/": Windows PowerShell 5.1 (CreateFromDirectory) would write "\" that other unzip tools
# read badly.
$z = [IO.Compression.ZipFile]::Open($zip, [IO.Compression.ZipArchiveMode]::Create)
try {
    foreach ($f in Get-ChildItem -Recurse -File $out) {
        $entry = "$name/" + $f.FullName.Substring($out.Length + 1).Replace('\', '/')
        [IO.Compression.ZipFileExtensions]::CreateEntryFromFile($z, $f.FullName, $entry, [IO.Compression.CompressionLevel]::Optimal) | Out-Null
    }
} finally { $z.Dispose() }
$z = [IO.Compression.ZipFile]::OpenRead($zip)
try {
    $inZip = $z.Entries | Where-Object { $_.Name } | ForEach-Object {
        [pscustomobject]@{ Path = ($_.FullName -replace '/', '\').Substring($name.Length + 1); Size = $_.Length; Packed = $_.CompressedLength }
    }
} finally { $z.Dispose() }
Test-Package $inZip

$zipSize = (Get-Item $zip).Length
Write-Host ''
Write-Host "Package: $zip ($([Math]::Round($zipSize / 1MB, 1)) MB)" -ForegroundColor Green
$inZip | ForEach-Object { '  {0,-28} {1,8:N0} KB' -f $_.Path, ($_.Size / 1KB) }

# ------------------------------------------------------------------ launcher screenshots (README)
if ($Screenshots) {
    $shots = if ([IO.Path]::IsPathRooted($Screenshots)) { $Screenshots } else { Join-Path $Root $Screenshots }
    Write-Host "==> Launcher screenshots in $shots (--preview)" -ForegroundColor Magenta
    $p = Start-Process -FilePath (Join-Path $out 'VIctory S.exe') -ArgumentList '--preview', "`"$shots`"" -WorkingDirectory $out -PassThru
    if (-not $p.WaitForExit(120000)) { Stop-Process -Id $p.Id; throw 'The launcher did not close: screenshots incomplete.' }
    Get-ChildItem $shots -Filter 'launcher-*.png' | ForEach-Object { '  {0,-28} {1,8:N0} KB' -f $_.Name, ($_.Length / 1KB) }
}
