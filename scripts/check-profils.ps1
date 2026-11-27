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
#
# Then the two options of 0.2 (low video memory mode, frame generation, "petite_vram" and "framegen" in
# reglages.json), on with a made-up frame generation folder (empty files): what they change, and with -Launcher the
# same comparison. Off (the rows above, with this PC's lanceur.json), nothing of theirs is there.
param([string]$Launcher = '')
$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot '..\series-s\reglages.ps1')

$GB = [double]1GB
function New-Pc($gpu, $vram, $threads, $ram) {
    [pscustomobject]@{ GpuName = $gpu; VramBytes = $vram * $GB; Threads = $threads; RamBytes = $ram * $GB }
}
# Machine, THIN/UNDER/VIEW/WORKERS threads, CARD_BUFFERS_MB, TEXTURE_MAX, WINDOWS_BAR_MB, DLSS, NVIDIA facts, warnings.
$rows = @(
    @{ Pc = New-Pc 'NVIDIA GeForce RTX 3050' 8 12 32;              Threads = '4/3/3/4'; Buf = '1024'; Tex = $null;  Bar = $null;  Dlss = $true;  Nv = $true;  Warn = 0 },
    @{ Pc = New-Pc 'NVIDIA GeForce GTX 1660 SUPER' 6 8 16;         Threads = '3/2/2/3'; Buf = '768';  Tex = '2048'; Bar = '3072'; Dlss = $false; Nv = $true;  Warn = 0 },
    @{ Pc = New-Pc 'NVIDIA GeForce RTX 4050 Laptop GPU' 6 16 16;   Threads = '4/3/3/4'; Buf = '768';  Tex = '2048'; Bar = '3072'; Dlss = $true;  Nv = $true;  Warn = 0 },
    @{ Pc = New-Pc 'NVIDIA GeForce RTX 4070' 12 20 32;             Threads = '6/4/4/6'; Buf = '2048'; Tex = $null;  Bar = $null;  Dlss = $true;  Nv = $true;  Warn = 0 },
    @{ Pc = New-Pc 'AMD Radeon RX 6600' 8 12 16;                   Threads = '3/2/2/3'; Buf = '1024'; Tex = $null;  Bar = '3072'; Dlss = $false; Nv = $false; Warn = 1 },
    @{ Pc = New-Pc 'Intel(R) UHD Graphics 770' 1 12 16;            Threads = '3/2/2/3'; Buf = '768';  Tex = '2048'; Bar = '3072'; Dlss = $false; Nv = $false; Warn = 1 },
    @{ Pc = New-Pc 'NVIDIA GeForce RTX 3080' 10 16 32;             Threads = '6/4/4/6'; Buf = '1024'; Tex = $null;  Bar = $null;  Dlss = $true;  Nv = $true;  Warn = 0 }
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

# ------------------------------------------------------------------ options: low video memory, frame generation
foreach ($k in @{ 'NVIDIA GeForce RTX 3050' = 'fsr3'; 'NVIDIA GeForce RTX 2060' = 'fsr3'; 'NVIDIA GeForce GTX 1660 SUPER' = 'fsr3'; 'NVIDIA GeForce RTX 3050 Ti Laptop GPU' = 'fsr3'
        'NVIDIA GeForce RTX 4070' = 'nvidia'; 'NVIDIA GeForce RTX 5090' = 'nvidia'; 'NVIDIA GeForce GTX 1080' = ''; 'NVIDIA RTX A2000' = ''; 'AMD Radeon RX 6600' = '' }.GetEnumerator()) {
    Test-Same "frame generation kind of $($k.Key)" (Get-FramegenKind $k.Key) $k.Value
}
Test-Same 'low video memory advised, 6 GB' (Get-HardwareProfile $rows[1].Pc).LowVramAdvised $true
Test-Same 'low video memory advised, 8 GB' (Get-HardwareProfile $rows[0].Pc).LowVramAdvised $false

$fgDir = Join-Path $env:TEMP 'victory-s-check-framegen'
New-Item -ItemType Directory -Force $fgDir | Out-Null
foreach ($f in @($Reglages.framegen.fichiers_requis) + @($Reglages.framegen.fichiers_dlss)) { Set-Content -LiteralPath (Join-Path $fgDir $f) '' }
$emptyDir = Join-Path $env:TEMP 'victory-s-check-framegen-empty'
New-Item -ItemType Directory -Force $emptyDir | Out-Null
function New-Choice($fg, $low, $mode) { [pscustomobject]@{ dlss_mode = $mode; dlss_dir = ''; framegen = $fg; petite_vram = $low } }
# Machine, frame generation, low video memory, DLSS mode, folder; then what the game gets ($null: absent).
$cases = @(
    @{ Pc = $rows[0].Pc; Fg = $false; Low = $true;  Mode = '';            Dir = $fgDir;    Want = @{ VICTORY_TEXTURE_MAX = '1024'; VICTORY_CARD_BUFFERS_MB = '512'; VICTORY_CARD_ROOM_FOR_TARGETS_MB = '768'; VICTORY_FRAMEGEN = $null } },
    @{ Pc = $rows[0].Pc; Fg = $true;  Low = $false; Mode = '';            Dir = $fgDir;    Want = @{ VICTORY_FRAMEGEN = '1'; VICTORY_FRAMEGEN_X = '2'; VICTORY_TEXTURE_ROOM_PERCENT = '90'; VICTORY_CARD_BUFFERS_MB = '768'; VICTORY_FRAMEGEN_DIR = $fgDir; VICTORY_DLSS_MODE = 'off'; VICTORY_DLSS_DIR = $null; VICTORY_RENDER_SCALE = '0.5'; VICTORY_TEXTURE_MAX = $null } },
    @{ Pc = $rows[0].Pc; Fg = $true;  Low = $true;  Mode = '';            Dir = $fgDir;    Want = @{ VICTORY_FRAMEGEN = '1'; VICTORY_CARD_BUFFERS_MB = '512'; VICTORY_TEXTURE_MAX = '1024'; VICTORY_TEXTURE_ROOM_PERCENT = '90' } },
    @{ Pc = $rows[0].Pc; Fg = $true;  Low = $false; Mode = 'performance'; Dir = $fgDir;    Want = @{ VICTORY_FRAMEGEN = '1'; VICTORY_DLSS_DIR = $fgDir; VICTORY_DLSS_MODE = 'performance'; VICTORY_RENDER_SCALE = $null; VICTORY_OWN_UPSCALER = $null } },
    @{ Pc = $rows[3].Pc; Fg = $true;  Low = $false; Mode = '';            Dir = $fgDir;    Want = @{ VICTORY_FRAMEGEN = '1'; VICTORY_CARD_BUFFERS_MB = '768' } },
    @{ Pc = $rows[4].Pc; Fg = $true;  Low = $false; Mode = '';            Dir = $fgDir;    Want = @{ VICTORY_FRAMEGEN = $null; VICTORY_DLSS_MODE = $null } },
    @{ Pc = $rows[0].Pc; Fg = $true;  Low = $false; Mode = '';            Dir = $emptyDir; Want = @{ VICTORY_FRAMEGEN = $null; VICTORY_TEXTURE_ROOM_PERCENT = $null; VICTORY_CARD_BUFFERS_MB = '1024' } }
)
foreach ($c in $cases) {
    $map = Get-GameEnvMap (New-Choice $c.Fg $c.Low $c.Mode) $c.Pc $c.Dir
    $what = "$($c.Pc.GpuName) framegen=$($c.Fg) low=$($c.Low) dlss='$($c.Mode)' dir=$(Split-Path -Leaf $c.Dir)"
    foreach ($k in $c.Want.Keys) { Test-Same "$what $k" $(if ($map.Set.Contains($k)) { $map.Set[$k] } else { $null }) $c.Want[$k] }
    if ($Launcher) {
        Remove-Item $tmp -ErrorAction SilentlyContinue
        $args_ = @('--print-env', "`"$tmp`"", '--ram-gb', [int]($c.Pc.RamBytes / $GB), '--vram-gb', [int]($c.Pc.VramBytes / $GB), '--threads', $c.Pc.Threads, '--gpu', "`"$($c.Pc.GpuName)`"", '--framegen-dir', "`"$($c.Dir)`"")
        $args_ += @('--framegen', $(if ($c.Fg) { 'on' } else { 'off' }), '--low-vram', $(if ($c.Low) { 'on' } else { 'off' }))
        if ($c.Mode) { $args_ += @('--dlss', '""', $c.Mode) } else { $args_ += @('--dlss', '""', 'off') }
        Start-Process -FilePath $Launcher -ArgumentList $args_ -Wait -WindowStyle Hidden
        if (-not (Test-Path $tmp)) { Write-Host "  MISMATCH: the launcher wrote nothing for $what" -ForegroundColor Red; $failed++; continue }
        $rust = @{}
        foreach ($line in Get-Content $tmp -Encoding UTF8) { $i = $line.IndexOf('='); if ($i -gt 0) { $rust[$line.Substring(0, $i)] = $line.Substring($i + 1) } }
        foreach ($k in $map.Set.Keys) { Test-Same "launcher $what $k" $rust[$k] $map.Set[$k] }
        foreach ($k in $map.Remove) { if ($rust.ContainsKey($k)) { Write-Host "  MISMATCH: $k should be removed ($what)" -ForegroundColor Red; $failed++ } }
        foreach ($k in $rust.Keys) { if ($k -like 'VICTORY_*' -and -not $map.Set.Contains($k) -and -not (Test-Path "env:$k")) { Write-Host "  MISMATCH: $k extra in the launcher ($what)" -ForegroundColor Red; $failed++ } }
    }
}
Remove-Item -Recurse -Force $fgDir, $emptyDir -ErrorAction SilentlyContinue
Write-Host "Options (low video memory, frame generation) checked on $($cases.Count) cases$(if ($Launcher) { ', with the graphical launcher' })."

if ($failed) { Write-Host "$failed mismatch(es)." -ForegroundColor Red; exit 1 }
Write-Host 'Profiles match.' -ForegroundColor Green
