# Checks the hardware profiles ("materiel" in series-s/reglages.json) on the console launcher side:
# series-s/reglages.ps1 must give, for made-up machines, the same settings as the Rust test
# launcher/src-tauri/src/profile.rs (synthetic_machines), and the reference PC must keep exactly the settings of
# the groups.
#
#   powershell -ExecutionPolicy Bypass -File scripts\check-profils.ps1
#   powershell -ExecutionPolicy Bypass -File scripts\check-profils.ps1 -Launcher <path of the graphical launcher>
#
# With -Launcher, each machine is also computed by the graphical launcher (--print-env) and the two environments
# are compared variable by variable. Exit code 1 at the first difference.
param([string]$Launcher = '')
$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot '..\series-s\reglages.ps1')

$GB = [double]1GB
function New-Pc($gpu, $vram, $threads, $ram) {
    [pscustomobject]@{ GpuName = $gpu; VramBytes = $vram * $GB; Threads = $threads; RamBytes = $ram * $GB }
}
# Machine, THIN/UNDER/VIEW/WORKERS threads, CARD_BUFFERS_MB, TEXTURE_MAX, WINDOWS_BAR_MB, DLSS, NVIDIA facts, warnings.
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
    if ([string]$got -ne [string]$want) { Write-Host "  MISMATCH ${what}: $got instead of $want" -ForegroundColor Red; $script:failed++ }
}

foreach ($r in $rows) {
    $map = Get-GameEnvMap $noDlss $r.Pc
    $prof = Get-HardwareProfile $r.Pc
    $e = $map.Set
    $th = (@('VICTORY_THIN_THREADS', 'VICTORY_UNDER_THREADS', 'VICTORY_VIEW_THREADS', 'VICTORY_PIPELINE_WORKERS') | ForEach-Object { $e[$_] }) -join '/'
    Write-Host ("{0,-48} threads {1}  buffers {2,-5} textures {3,-5} BAR {4,-5} DLSS {5}" -f $prof.Label, $th, $e['VICTORY_CARD_BUFFERS_MB'], $e['VICTORY_TEXTURE_MAX'], $e['VICTORY_WINDOWS_BAR_MB'], $prof.Dlss)
    Test-Same 'threads' $th $r.Threads
    Test-Same 'CARD_BUFFERS_MB' $e['VICTORY_CARD_BUFFERS_MB'] $r.Buf
    Test-Same 'TEXTURE_MAX' $e['VICTORY_TEXTURE_MAX'] $r.Tex
    Test-Same 'WINDOWS_BAR_MB' $e['VICTORY_WINDOWS_BAR_MB'] $r.Bar
    Test-Same 'DLSS' $prof.Dlss $r.Dlss
    Test-Same 'NVIDIA facts' $prof.ShadersNvidia $r.Nv
    Test-Same 'warnings' $prof.Warnings.Count $r.Warn
    if ($r.Pc.VramBytes -lt 11 * $GB) { if ($e['VICTORY_CARD_BUFFERS_MB'] -eq '2048') { Write-Host '  MISMATCH: 2048 MB under 11 GB' -ForegroundColor Red; $failed++ } }
}

# The reference PC: exactly the groups of reglages.json, nothing added.
$ref = Get-HardwareProfile $rows[0].Pc
Test-Same 'reference, changed variables' $ref.Env.Count 0
$groups = Get-SeriesSSettings
$set = (Get-GameEnvMap $noDlss $rows[0].Pc).Set
foreach ($k in $groups.Keys) { Test-Same "reference $k" $set[$k] $groups[$k] }
Test-Same 'reference, number of variables' $set.Count ($groups.Count + 2)

# The graphical launcher must give the same environment (same player settings, lanceur.json).
if ($Launcher) {
    $s = Complete-Settings (Get-Settings)
    $tmp = Join-Path $env:TEMP 'victory-s-check-env.txt'
    foreach ($r in $rows) {
        Remove-Item $tmp -ErrorAction SilentlyContinue
        $args_ = @('--print-env', "`"$tmp`"", '--ram-gb', [int]($r.Pc.RamBytes / $GB), '--vram-gb', [int]($r.Pc.VramBytes / $GB), '--threads', $r.Pc.Threads, '--gpu', "`"$($r.Pc.GpuName)`"")
        Start-Process -FilePath $Launcher -ArgumentList $args_ -Wait -WindowStyle Hidden
        if (-not (Test-Path $tmp)) { Write-Host "  MISMATCH: the launcher wrote nothing for $($r.Pc.GpuName)" -ForegroundColor Red; $failed++; continue }
        $rust = @{}
        foreach ($line in Get-Content $tmp -Encoding UTF8) { $i = $line.IndexOf('='); if ($i -gt 0) { $rust[$line.Substring(0, $i)] = $line.Substring($i + 1) } }
        $map = Get-GameEnvMap $s $r.Pc
        foreach ($k in $map.Set.Keys) { Test-Same "launcher $($r.Pc.GpuName) $k" $rust[$k] $map.Set[$k] }
        foreach ($k in $map.Remove) { if ($rust.ContainsKey($k)) { Write-Host "  MISMATCH: $k should be removed ($($r.Pc.GpuName))" -ForegroundColor Red; $failed++ } }
        foreach ($k in $rust.Keys) { if ($k -like 'VICTORY_*' -and -not $map.Set.Contains($k) -and -not (Test-Path "env:$k")) { Write-Host "  MISMATCH: $k extra in the launcher ($($r.Pc.GpuName))" -ForegroundColor Red; $failed++ } }
    }
    Write-Host "Graphical launcher compared on $($rows.Count) machines."
}

if ($failed) { Write-Host "$failed mismatch(es)." -ForegroundColor Red; exit 1 }
Write-Host 'Profiles match.' -ForegroundColor Green
