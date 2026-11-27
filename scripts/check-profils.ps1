# Vérifie les profils du matériel (« materiel » dans series-s/reglages.json) côté lanceur console :
# series-s/reglages.ps1 doit donner, pour des machines inventées, les mêmes réglages que le test Rust
# launcher/src-tauri/src/profile.rs (synthetic_machines), et le PC de référence doit garder exactement les réglages
# des groupes.
#
#   powershell -ExecutionPolicy Bypass -File scripts\check-profils.ps1
#   powershell -ExecutionPolicy Bypass -File scripts\check-profils.ps1 -Launcher <chemin du lanceur graphique>
#
# Avec -Launcher, chaque machine est aussi calculée par le lanceur graphique (--print-env) et les deux
# environnements sont comparés variable par variable. Code de sortie 1 au premier écart.
param([string]$Launcher = '')
$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot '..\series-s\reglages.ps1')

$GB = [double]1GB
function New-Pc($gpu, $vram, $threads, $ram) {
    [pscustomobject]@{ GpuName = $gpu; VramBytes = $vram * $GB; Threads = $threads; RamBytes = $ram * $GB }
}
# Machine, fils THIN/UNDER/VIEW/WORKERS, CARD_BUFFERS_MB, TEXTURE_MAX, WINDOWS_BAR_MB, DLSS, faits NVIDIA, avertissements.
$rows = @(
    @{ Pc = New-Pc 'NVIDIA GeForce RTX 3050' 8 12 32;              Threads = '4/3/3/4'; Buf = '1536'; Tex = $null;  Bar = $null;  Dlss = $true;  Nv = $true;  Warn = 0 },
    @{ Pc = New-Pc 'NVIDIA GeForce GTX 1660 SUPER' 6 8 16;         Threads = '3/2/2/3'; Buf = '768';  Tex = '2048'; Bar = '3072'; Dlss = $false; Nv = $true;  Warn = 0 },
    @{ Pc = New-Pc 'NVIDIA GeForce RTX 4050 Laptop GPU' 6 16 16;   Threads = '4/3/3/4'; Buf = '768';  Tex = '2048'; Bar = '3072'; Dlss = $true;  Nv = $true;  Warn = 0 },
    @{ Pc = New-Pc 'NVIDIA GeForce RTX 4070' 12 20 32;             Threads = '6/4/4/6'; Buf = '2048'; Tex = $null;  Bar = $null;  Dlss = $true;  Nv = $true;  Warn = 0 },
    @{ Pc = New-Pc 'AMD Radeon RX 6600' 8 12 16;                   Threads = '3/2/2/3'; Buf = '1024'; Tex = $null;  Bar = '3072'; Dlss = $false; Nv = $false; Warn = 1 },
    @{ Pc = New-Pc 'Intel(R) UHD Graphics 770' 1 12 16;            Threads = '3/2/2/3'; Buf = '768';  Tex = '2048'; Bar = '3072'; Dlss = $false; Nv = $false; Warn = 1 },
    @{ Pc = New-Pc 'NVIDIA GeForce RTX 3080' 10 16 32;             Threads = '6/4/4/6'; Buf = '1536'; Tex = $null;  Bar = $null;  Dlss = $true;  Nv = $true;  Warn = 0 }
)

$noDlss = [pscustomobject]@{ dlss_mode = ''; dlss_dir = '' }
$failed = 0
function Test-Same($what, $got, $want) {
    if ([string]$got -ne [string]$want) { Write-Host "  ÉCART $what : $got au lieu de $want" -ForegroundColor Red; $script:failed++ }
}

foreach ($r in $rows) {
    $map = Get-GameEnvMap $noDlss $r.Pc
    $prof = Get-HardwareProfile $r.Pc
    $e = $map.Set
    $th = (@('VICTORY_THIN_THREADS', 'VICTORY_UNDER_THREADS', 'VICTORY_VIEW_THREADS', 'VICTORY_PIPELINE_WORKERS') | ForEach-Object { $e[$_] }) -join '/'
    Write-Host ("{0,-48} fils {1}  tampons {2,-5} textures {3,-5} BAR {4,-5} DLSS {5}" -f $prof.Label, $th, $e['VICTORY_CARD_BUFFERS_MB'], $e['VICTORY_TEXTURE_MAX'], $e['VICTORY_WINDOWS_BAR_MB'], $prof.Dlss)
    Test-Same 'fils' $th $r.Threads
    Test-Same 'CARD_BUFFERS_MB' $e['VICTORY_CARD_BUFFERS_MB'] $r.Buf
    Test-Same 'TEXTURE_MAX' $e['VICTORY_TEXTURE_MAX'] $r.Tex
    Test-Same 'WINDOWS_BAR_MB' $e['VICTORY_WINDOWS_BAR_MB'] $r.Bar
    Test-Same 'DLSS' $prof.Dlss $r.Dlss
    Test-Same 'faits NVIDIA' $prof.ShadersNvidia $r.Nv
    Test-Same 'avertissements' $prof.Warnings.Count $r.Warn
    if ($r.Pc.VramBytes -lt 11 * $GB) { if ($e['VICTORY_CARD_BUFFERS_MB'] -eq '2048') { Write-Host '  ÉCART : 2048 Mo sous 11 Go' -ForegroundColor Red; $failed++ } }
}

# Le PC de référence : exactement les groupes de reglages.json, rien d'ajouté.
$ref = Get-HardwareProfile $rows[0].Pc
Test-Same 'référence, variables changées' $ref.Env.Count 0
$groups = Get-SeriesSSettings
$set = (Get-GameEnvMap $noDlss $rows[0].Pc).Set
foreach ($k in $groups.Keys) { Test-Same "référence $k" $set[$k] $groups[$k] }
Test-Same 'référence, nombre de variables' $set.Count ($groups.Count + 2)

# Le lanceur graphique doit donner le même environnement (mêmes réglages du joueur, lanceur.json).
if ($Launcher) {
    $s = Complete-Settings (Get-Settings)
    $tmp = Join-Path $env:TEMP 'victory-s-check-env.txt'
    foreach ($r in $rows) {
        Remove-Item $tmp -ErrorAction SilentlyContinue
        $args_ = @('--print-env', "`"$tmp`"", '--ram-gb', [int]($r.Pc.RamBytes / $GB), '--vram-gb', [int]($r.Pc.VramBytes / $GB), '--threads', $r.Pc.Threads, '--gpu', "`"$($r.Pc.GpuName)`"")
        Start-Process -FilePath $Launcher -ArgumentList $args_ -Wait -WindowStyle Hidden
        if (-not (Test-Path $tmp)) { Write-Host "  ÉCART : le lanceur n'a rien écrit pour $($r.Pc.GpuName)" -ForegroundColor Red; $failed++; continue }
        $rust = @{}
        foreach ($line in Get-Content $tmp -Encoding UTF8) { $i = $line.IndexOf('='); if ($i -gt 0) { $rust[$line.Substring(0, $i)] = $line.Substring($i + 1) } }
        $map = Get-GameEnvMap $s $r.Pc
        foreach ($k in $map.Set.Keys) { Test-Same "lanceur $($r.Pc.GpuName) $k" $rust[$k] $map.Set[$k] }
        foreach ($k in $map.Remove) { if ($rust.ContainsKey($k)) { Write-Host "  ÉCART : $k devrait être retiré ($($r.Pc.GpuName))" -ForegroundColor Red; $failed++ } }
        foreach ($k in $rust.Keys) { if ($k -like 'VICTORY_*' -and -not $map.Set.Contains($k) -and -not (Test-Path "env:$k")) { Write-Host "  ÉCART : $k en plus dans le lanceur ($($r.Pc.GpuName))" -ForegroundColor Red; $failed++ } }
    }
    Write-Host "Lanceur graphique comparé sur $($rows.Count) machines."
}

if ($failed) { Write-Host "$failed écart(s)." -ForegroundColor Red; exit 1 }
Write-Host 'Profils conformes.' -ForegroundColor Green
