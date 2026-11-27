# Starts the title on an NVIDIA card with the Series S settings measured on an RTX 5070 (about 18-22 fps, no device loss).
#   scripts\play-nvidia.ps1 -Exe <path of your own copy's executable>
# -NoSmoothMotion  run target\<build>\victory.exe itself. By default a copy named victory_sm.exe is started instead,
#                  so a Smooth Motion profile set for that exe in the NVIDIA App applies every time.
# -SeriesX         the Series X profile instead (slower: about 7 fps at half scale)
# -Quality         fewer stretched/jumping bodies and less rainbow skin, at about 13 fps instead of about 20
# -RayTracing      keep the title's ray-tracing passes (off by default: see below)
# -Resolution      the height the game renders at before the title's own upscaler: 2160, 1440, 1080, 720, 540, 480 or 432.
#                  432 (render scale 0.4) on the RTX 3060: 13.6 -> 15.0 fps on average, a softer picture.
#                  Series S outputs 1080p (so up to 1080 there), Series X 2160p. Default: half the output (540 on S,
#                  1080 on X). The play-S-*.ps1 and play-X-*.ps1 scripts next to this one call it with each.
# -Adapter         on a PC with more than one card, the one to use: part of its name (-Adapter 5060) or its index
#                  (-Adapter '#1'). Default: Windows' default card (the one the main display is on).
# -Build           target\diag (the default, the build all measurements are of) or target\release
#                  or target\perf (scripts\build-perf.ps1: no diagnostics, fat LTO, built for this CPU). The skip list
#                  and VICTORY_KEYS work in every build since 26/11.
# -More            other settings: -More @{VICTORY_PANEL='1'}
# -Capture         record one frame for `victory replay` (docs/replay.md) into %LOCALAPPDATA%\victory\captures:
#                  when the picture shows what is wrong, run scripts\capture-now.ps1 in another window (about 9 GB,
#                  the game stands still for a few seconds). Diagnostics build only.
# -SixCores        helper thread pools sized for a 6-core / 12-thread CPU (on by default with 12 threads or fewer)
# -SmallOnCard     title buffers from 64 KiB (not 1 MiB) compared on the card (on by default)
# -Classic         neither of the two above: the settings as measured on the RTX 5070
# Everything the run prints is also kept in %LOCALAPPDATA%\victory\logs\nvidia-<date>.log.
# Run it from a normal window, not as administrator: the driver then keeps its shader cache elsewhere.
param(
    [Parameter(Mandatory)][string]$Exe,
    [switch]$Capture,
    [switch]$SixCores,
    [switch]$SmallOnCard,
    [switch]$Classic,
    [switch]$NoSmoothMotion,
    [switch]$SeriesX,
    [switch]$Quality,
    [switch]$RayTracing,
    [ValidateSet('2160', '1440', '1080', '720', '540', '480', '432')][string]$Resolution,
    [string]$Adapter,
    [ValidateSet('diag', 'release', 'perf')][string]$Build = 'diag',
    [hashtable]$More = @{}
)
$settings = @{
    # This fork's play set (scripts\play.ps1, not -Safe): the speed set.
    VICTORY_TEXTURE_NO_WAIT = '1'; VICTORY_RELAY = '2'; VICTORY_CARD_STEPS = '2'
    VICTORY_WIDE_BACK = '1'; VICTORY_CARD_REHASH_RUNS = '1'
    VICTORY_JOIN_COPIES = '1'; VICTORY_INDIRECT_WINDOW = '1'; VICTORY_RELAY_COPY = '1'
    VICTORY_INDIRECT_BATCH = '2'; VICTORY_MAP_THREADS = '1'; VICTORY_TITLE_PRIORITIES = '0'; VICTORY_CARD_AWAKE = '1'
    VICTORY_SHOW_AFTER = '1'; VICTORY_AMBIENT = '0.8'; VICTORY_D3D12_DRED = 'light'; VICTORY_SKIP_PIPELINES = 'none'
    VICTORY_THIN = '1'; VICTORY_JOIN_PARTS = '8'; VICTORY_UPLOAD_AHEAD = '1'
    # Cheap picture fixes, on in both sets:
    # - Overbright views: without rays the light probes keep the sky's light indoors. The fog is bounded by 1.5 times
    #   the light around, and what the title left by 2 times the estimate (was 4; ambient.rs).
    VICTORY_AMBIENT_AIR = '1.5'; VICTORY_AMBIENT_MOST = '2'
    # - Black blobs: a NaN a shader writes is made 0 before blur and the temporal upscaler spread it (emit/lower.rs).
    VICTORY_NAN_SCRUB = '1'
    # Wait for new shaders rather than skip their draws (a hitch the first time a place is seen). Each pipeline is
    # waited for at most once (VICTORY_PIPELINE_WAIT_ONCE, on by default): one that takes longer no longer stops the
    # picture in every frame it is in view (pipeline/make.rs).
    VICTORY_PIPELINE_WAIT_MS = '10000'
    # NVIDIA caps the shader-visible view heap at 1,000,000: without more dynamic slots thousands of draws a frame
    # are dropped (descriptor/host.rs dynamic_slots).
    VICTORY_DYNAMIC_SLOTS = '524288'
    # Half render scale with the title's own upscaler (Series S needs it with a scale below 1; victory-loader hooks.rs).
    VICTORY_RENDER_SCALE = '0.5'; VICTORY_OWN_UPSCALER = '1'
    # Ray walks end at the root while the structure builder is off (emit/intersect.rs).
    VICTORY_BVH_MISS = '1'
    # Memory compares on the card instead of the CPU: 11 -> about 22 fps on Series S.
    VICTORY_CARD_BUFFERS_MIN_MB = '1'; VICTORY_CARD_BUFFERS_MB = '1536'
    # Series S presents the interface as a second plane: laid over the game (compose.rs), else black before gameplay.
    VICTORY_COMPOSE = '1'
    # Windows of title memory outside buffers in card memory the CPU maps (resizable BAR), up to 3 GB, while the
    # card has room (shadow.rs window_mapped_for). Otherwise they live in the PC's memory shared with the card,
    # which Windows caps at half the RAM: on a 16 GB PC (RTX 3060) the shared memory reached 7.5 GB about a minute
    # into the world and every run ended in 0x8007000E, then a fault in the driver. With this: no crash, the shared
    # memory flat at about 4.4 GB, 10-12 fps instead of 2-3. Without a resizable BAR it changes nothing.
    VICTORY_WINDOWS_BAR_MB = '3072'
}
# Ray tracing mostly off. The structure builder cannot run on NVIDIA (its 64-lane programs are refused; upstream
# Marvisak/VIctory pull request 2311), so every ray misses, and the passes that turn misses into light made rooms go
# white depending on the view angle. rt_off_hashes.txt lists the 1,000 CS_RaytracingResolve and 34 traced-shadow
# shaders, which are skipped. The screen-probe and visibility traces keep running: skipping those too left their
# outputs as garbage (rainbow glitter everywhere).
$rtList = Join-Path $PSScriptRoot 'rt_off_hashes.txt'
if (-not $RayTracing -and (Test-Path $rtList)) {
    $settings.VICTORY_SKIP_PIPELINES = (Get-Content $rtList -Raw).Trim()
}
if ($Quality) {
    # Stretched/jumping bodies: relay stage 1 and no card steps (avatarmanz/VIctory-7900XTX issue 13,
    # fxceless/victory-fork issue 89). Rainbow skin: texture refreshes wait for the runtime's remapping.
    # Costs about 7 fps (the submitting thread waits about 30 ms a frame for the card).
    $settings.VICTORY_RELAY = '1'; $settings.VICTORY_CARD_STEPS = '0'; $settings.VICTORY_TEXTURE_NO_WAIT = '0'
}
# The render scale is the rendered height over the output height (Series S 1080, Series X 2160).
$outHeight = if ($SeriesX) { 2160 } else { 1080 }
$console = if ($SeriesX) { 'X' } else { 'S' }
if ($Resolution) {
    if ([int]$Resolution -gt $outHeight) {
        Write-Host "Series $console outputs ${outHeight}p: ${Resolution}p is above it."
        return
    }
    $settings.VICTORY_RENDER_SCALE = ([int]$Resolution / $outHeight).ToString('0.####', [Globalization.CultureInfo]::InvariantCulture)
}
if ($Adapter) { $settings.VICTORY_ADAPTER = $Adapter }
# Measured on a Ryzen 5 5600X + RTX 3060 (26/11, three 6-minute runs in one place, scriptsench-3060.ps1):
# as it was 13.0 fps (preparing thread 72 ms a frame, copying back 26-28 ms); with both below 14.0 fps and the end
# of the run at 15-16 fps (preparing 55 ms, copying back 15.6 ms, list translation 263 ms of CPU instead of 400).
# On by default on CPUs of up to 12 threads (-SixCores forces it); -Classic: the settings as they were.
$small = [Environment]::ProcessorCount -le 12
if (($small -or $SixCores) -and -not $Classic) {
    # The helper pools are sized for a 24-thread i9 (list translation 8, title bytes under shader writes 6,
    # descriptor records 6, picked views 4, pipeline workers two thirds of the threads). On a 12-thread
    # CPU they share 6 cores with the title's own threads and the preparing thread: fewer of each.
    $settings.VICTORY_THIN_THREADS = '5'; $settings.VICTORY_UNDER_THREADS = '3'
    $settings.VICTORY_VIEW_THREADS = '3'; $settings.VICTORY_PIPELINE_WORKERS = '4'
}
if (-not $Classic) {
    # Up to 16 submissions (96 lists) prepared together instead of 8 (48): per-submission work off the preparing
    # thread. RTX 3060, 26/11, 6-minute runs: 13.6 -> 14.2 fps on average, 1% low 7.8 -> 9.2 fps, P99 121 -> 103 ms.
    $settings.VICTORY_JOIN_PARTS = '16'; $settings.VICTORY_JOIN_LISTS = '96'
}
if (-not $Classic -or $SmallOnCard) {
    # Every title buffer from 64 KiB up gets its copy and compare on the card, not only those of 1 MiB and more
    # (VICTORY_CARD_BUFFERS_MIN_KB, after fxceless c36971b2): copying back 26-28 -> 15.6 ms a frame on the 3060.
    $settings.VICTORY_CARD_BUFFERS_MIN_KB = '64'
}
foreach ($k in $More.Keys) { $settings[$k] = $More[$k] }
$scale = [double]::Parse($settings.VICTORY_RENDER_SCALE, [Globalization.CultureInfo]::InvariantCulture)
Write-Host "Series $console, rendering at $([int]($outHeight * $scale))p (render scale $($settings.VICTORY_RENDER_SCALE))"

$victory = Join-Path $PSScriptRoot "..\target\$Build\victory.exe"
if (-not (Test-Path $victory)) { Write-Host "No $victory. Build it first (README.md)."; return }
if (-not (Test-Path $Exe)) { Write-Host "No $Exe."; return }
if (-not $NoSmoothMotion) {
    # A copy under the name the NVIDIA App's Smooth Motion profile is set for, refreshed when the build changes.
    $sm = Join-Path (Split-Path $victory) 'victory_sm.exe'
    if (-not (Test-Path $sm) -or (Get-Item $sm).LastWriteTime -lt (Get-Item $victory).LastWriteTime) {
        try { Copy-Item $victory $sm -Force -ErrorAction Stop } catch { Write-Host "victory_sm.exe could not be refreshed ($($_.Exception.Message))" }
    }
    # A copy older than the build (it was running when the build changed): run the build itself.
    if ((Test-Path $sm) -and (Get-Item $sm).LastWriteTime -ge (Get-Item $victory).LastWriteTime) {
        $victory = $sm
    } else {
        Write-Host "victory_sm.exe is older than the build: running victory.exe (no Smooth Motion profile this time)"
    }
}
# (Other settings left over in this window go; where your files are stays.)
$paths = 'VICTORY_CONFIG', 'VICTORY_GAME_DIR', 'VICTORY_CACHE_DIR', 'VICTORY_DATA_DIR'
Get-ChildItem Env:VICTORY_* | Where-Object { $_.Name -notin $paths } | ForEach-Object { Remove-Item "Env:$($_.Name)" }
if ($Capture) {
    $captures = Join-Path $env:LOCALAPPDATA 'victory\captures'
    New-Item -ItemType Directory -Force $captures | Out-Null
    Remove-Item (Join-Path $captures 'go') -ErrorAction SilentlyContinue
    $settings.VICTORY_RECORD = $captures
    Write-Host "capture: run scripts\capture-now.ps1 when the picture shows the problem ($captures)"
}
foreach ($k in $settings.Keys) { Set-Item "Env:$k" $settings[$k] }
$Exe = (Resolve-Path $Exe).Path
$runArgs = @('run', $Exe, '--execute')
if (-not $SeriesX) { $runArgs += @('--console', 'series-s') }
$logs = Join-Path $env:LOCALAPPDATA 'victory\logs'
New-Item -ItemType Directory -Force $logs | Out-Null
$log = Join-Path $logs ('nvidia-{0}.log' -f (Get-Date -Format 'yyyyMMdd-HHmmss'))
# the settings first, so a log says what it was run with
# (UTF-8 throughout: Tee-Object of Windows PowerShell 5.1 appends UTF-16.)
$writer = [IO.StreamWriter]::new($log, $false, [Text.UTF8Encoding]::new($false))
$settings.Keys | Sort-Object | ForEach-Object { $writer.WriteLine("# $_=$($settings[$_])") }
$writer.WriteLine("# exe=$victory")
$writer.Flush()
Write-Host "log: $log"
Push-Location (Join-Path $PSScriptRoot '..')   # victory.toml is looked for here
try {
    & $victory @runArgs 2>&1 | ForEach-Object { $line = "$_"; $writer.WriteLine($line); $line }
} finally {
    $writer.Dispose()
    Pop-Location
    Write-Host "log saved: $log"
}
