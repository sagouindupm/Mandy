# Sagouindupm | VIctory: launcher for beginners (Series S profile, NVIDIA cards with 6-12 GB, 6-core CPU).
# 1. First launch: asks where the game is, writes the configuration.
# 2. [2] Prepares the shaders: translates all the game's shaders once (15-30 min).
# 3. [3] Builds the pipelines outside the game, even before the first session: imports the published list
#    (series-s\pipelines\series-s-rtx.txt, no game code) with the shaders of YOUR copy, then victory warm.
# 4. [1] Play: each session also records the pipelines the game creates (VICTORY_PIPELINE_BOOK); [3] builds them.
# Not 'Stop': under Windows PowerShell 5.1, every line the game writes to its error output becomes an error, and
# the launcher stopped at the first line of the log.
$ErrorActionPreference = 'Continue'
# Paths and lanceur.json: series-s\reglages.ps1, which applies series-s\reglages.json (Series S settings,
# ray tracing, 24 GB rule, DLSS; the same as for the graphical launcher).
. (Join-Path $PSScriptRoot 'reglages.ps1')

function Show-Title {
    Clear-Host
    $Host.UI.RawUI.WindowTitle = 'Sagouindupm | VIctory'
    Write-Host ''
    Write-Host '   ███  Sagouindupm | VIctory  ███' -ForegroundColor Magenta
    Write-Host '   Series S profile - tuned for low-end PCs' -ForegroundColor DarkMagenta
    Write-Host ''
}

function Initialize-Game($s) {
    # A complete copy: the exe with its archives next to it ("jeu" in reglages.json), not the exe alone.
    while (-not ($s.game_exe -and (Test-Path $s.game_exe) -and -not ($problem = Test-GameCopy $s.game_exe))) {
        if ($s.game_exe -and $problem) { Write-Host $problem -ForegroundColor Red; $problem = $null }
        Write-Host "Where is the game?" -ForegroundColor Yellow
        Write-Host "Drag here the game_scarlett_master.exe file of YOUR (complete) copy of the game, then Enter."
        $p = (Read-Host 'Path').Trim('"', ' ')
        if ($p -and (Test-Path $p) -and $p.EndsWith('.exe')) { $s.game_exe = $p } else { Write-Host 'File not found, try again.' -ForegroundColor Red }
    }
    Write-VictoryToml $s
}

function Set-Dlss($s) {
    if (-not (Get-HardwareProfile (Get-Hardware)).Dlss) { Write-Host 'DLSS: needs an NVIDIA RTX card. Here, 540p and the game''s upscaler.' -ForegroundColor Yellow; return }
    Write-Host 'DLSS (NVIDIA RTX cards): you need NVIDIA''s Streamline DLLs (sl.interposer.dll, sl.common.dll,' -ForegroundColor Yellow
    Write-Host 'sl.dlss.dll, nvngx_dlss.dll), to download yourself: https://github.com/NVIDIA-RTX/Streamline/releases'
    Write-Host '(bin\x64 folder of the archive). They are not shipped here (NVIDIA license).'
    $p = (Read-Host "Folder that contains sl.interposer.dll (empty = keep '$($s.dlss_dir)')").Trim('"', ' ')
    if ($p) {
        if (Test-Path (Join-Path $p 'sl.interposer.dll')) { $s.dlss_dir = $p } else { Write-Host 'sl.interposer.dll not found in this folder.' -ForegroundColor Red; return }
    }
    Write-Host '  [1] Ultra performance (fastest, advised on 6-8 GB)'
    Write-Host '  [2] Performance'
    Write-Host '  [3] Quality (best-looking, slower)'
    Write-Host '  [4] Off (the game''s upscaler)'
    $s.dlss_mode = switch ((Read-Host 'Mode').Trim()) { '1' { 'ultra' } '2' { 'performance' } '3' { 'quality' } default { 'off' } }
    Save-Settings $s
    Write-Host "DLSS: $($s.dlss_mode)" -ForegroundColor Green
}

function Start-Game($s) {
    Set-GameEnv $s
    if (-not $s.shaders_ready) {
        Write-Host 'Shaders not prepared yet: the first minutes will be slow. Advice: [2] then [3] first.' -ForegroundColor Yellow
    }
    $log = Join-Path $Logs ("partie_{0:yyyy-MM-dd_HHmm}.log" -f (Get-Date))
    Write-Host "`nLaunching... (log: $log)" -ForegroundColor Magenta
    Write-Host 'After a session, [3] also builds the recorded pipelines the published list did not have.'
    # Through cmd: PowerShell 5.1 does not turn the log lines into errors.
    & cmd.exe /c "`"$Exe`" run `"$($s.game_exe)`" --console series-s --execute > `"$log`" 2>&1"
    $s.parties++; Save-Settings $s
    Write-Host "`nSession over (code $LASTEXITCODE)."
}

function Initialize-Shaders($s) {
    Write-Host 'Preparing the shaders ahead of time avoids black screens and stutter.' -ForegroundColor Yellow
    Write-Host 'You need a folder of shaders extracted from YOUR copy of the game (or its shaders.rpf).'
    if (-not ($s.shaders_dir -and (Test-Path $s.shaders_dir))) {
        $p = (Read-Host 'Shaders folder (empty = cancel)').Trim('"', ' ')
        if (-not $p -or -not (Test-Path $p)) { Write-Host 'Cancelled.'; return }
        $s.shaders_dir = $p; Save-Settings $s
    }
    Set-GameEnv $s
    # The card's facts (NVIDIA: waves of 32, 1,000,000 views): without them the cache is useless in game.
    # NVIDIA only: an AMD card keeps the generic Series S translation.
    if ((Get-HardwareProfile (Get-Hardware)).ShadersNvidia) {
        $hostEnv = Get-ShaderHostEnv; foreach ($k in $hostEnv.Keys) { Set-Item "env:$k" $hostEnv[$k] }
    }
    # The shared list first: the translation keeps along the way the shaders of your files that it names ([3]).
    $list = Get-SharedPipelineList
    if ($list) { & $Exe pipelines import $list | Tee-Object -FilePath (Join-Path $Logs 'pipelines_import.txt') }
    Write-Host "`nTranslating all the shaders (15 to 30 minutes, the PC will be very busy)..." -ForegroundColor Magenta
    $out = & $Exe shaders $s.shaders_dir --jobs (Get-ShaderJobs) | Tee-Object -FilePath (Join-Path $Logs 'shaders_traduction.txt')
    $code = $LASTEXITCODE
    $out | Select-Object -Last 3
    # A complete copy gives ~25,000; a few hundred means an incomplete copy.
    $n = Get-ShadersCount $out
    if ($code -ne 0) { Write-Host "The translation stopped (code $code), see $Logs." -ForegroundColor Red }
    elseif ($null -ne $n -and $n -lt [int]$Reglages.jeu.shaders_min) {
        $s.shaders_ready = $false; Save-Settings $s
        Write-Host "Only $n shaders translated: a complete copy of the game gives ~25,000. Check the game copy ([5]) and the shaders folder, then start again." -ForegroundColor Red
    }
    else { $s.shaders_ready = $true; Save-Settings $s; Write-Host "Shaders ready$(if ($null -ne $n) { " ($n)" })!" -ForegroundColor Green }
}

function Initialize-Pipelines($s) {
    Set-GameEnv $s
    # The shared list: the pipelines the game uses until after the boat mission (keys, shader hashes, states; no
    # game code). The shaders themselves come from YOUR shaders folder: kept by [2]; otherwise --shaders looks
    # for them there (~7 min; nothing to do if they are already there).
    $list = Get-SharedPipelineList
    if ($list) {
        Write-Host "`nImporting the shared pipeline list..." -ForegroundColor Magenta
        $import = @('pipelines', 'import', $list)
        if ($s.shaders_dir -and (Test-Path $s.shaders_dir)) { $import += @('--shaders', $s.shaders_dir) }
        else { Write-Host 'Shaders folder unknown: do [2] first, otherwise only the pipelines from your sessions will be built.' -ForegroundColor Yellow }
        & $Exe @import | Tee-Object -FilePath (Join-Path $Logs 'pipelines_import.txt')
    }
    Write-Host "`nBuilding the pipelines (shared list and your sessions, a few minutes)..." -ForegroundColor Magenta
    & $Exe warm --used | Tee-Object -FilePath (Join-Path $Logs 'pipelines_warm.txt') | Select-Object -Last 2
    # Mark read by the graphical launcher: pipelines built at this number of sessions.
    # Mark of the shared list that was built (read by the graphical launcher, step 4).
    if ($LASTEXITCODE -eq 0) { $s.pipelines_parties = $s.parties; $s.pipelines_liste = Get-SharedPipelineMark; Save-Settings $s }
    Write-Host 'Pipelines ready: the next session starts without stutter.' -ForegroundColor Green
}

function Backup-Saves {
    $data = Join-Path $env:LOCALAPPDATA 'victory\data'
    if (-not (Test-Path $data)) { Write-Host 'No save found yet.'; return }
    $dst = Join-Path $Home_ ("sauvegardes\{0:yyyy-MM-dd_HHmmss}" -f (Get-Date))
    Copy-Item -Recurse $data $dst
    Write-Host "Saves copied to $dst" -ForegroundColor Green
}

if (-not (Test-Path $Exe)) {
    Show-Title
    Write-Host "victory.exe not found ($Exe). Build it first: cargo build --release --locked -p victory" -ForegroundColor Red
    Write-Host '(or download the ready-made version: https://github.com/sagouindupm/Mandy/releases)'
    Read-Host 'Enter to quit'; exit 1
}
$s = Complete-Settings (Get-Settings)
Show-Title
Initialize-Game $s
# Profile picked from this PC ("materiel" in reglages.json), the same as in the graphical launcher.
$Profil = Get-HardwareProfile (Get-Hardware)
while ($true) {
    Show-Title
    $state = if ($s.shaders_ready) { 'ready' } else { 'not prepared yet' }
    $dlss = if ($s.dlss_mode -and $s.dlss_mode -ne 'off') { $s.dlss_mode } else { 'off' }
    Write-Host "  Detected profile: Series S · $($Profil.Label)" -ForegroundColor DarkMagenta
    foreach ($w in $Profil.Warnings) { Write-Host "  ! $w" -ForegroundColor Yellow }
    foreach ($t in $Profil.Tips) { Write-Host "  $t" -ForegroundColor DarkYellow }
    Write-Host "  Shaders: $state   |   DLSS: $dlss   |   Sessions launched: $($s.parties)`n"
    Write-Host '  [1] Play'
    Write-Host '  [2] Prepare the shaders ahead of time (once, recommended)'
    Write-Host '  [3] Build the pipelines (after [2], before the first session; redo after a session)'
    Write-Host '  [4] Copy my saves to a safe place'
    Write-Host '  [5] Change the game location'
    Write-Host '  [6] DLSS (NVIDIA RTX cards)'
    Write-Host '  [Q] Quit'
    switch ((Read-Host "`nChoice").Trim().ToUpper()) {
        '1' { Start-Game $s; Read-Host 'Enter to go back to the menu' | Out-Null }
        '2' { Initialize-Shaders $s; Read-Host 'Enter to go back to the menu' | Out-Null }
        '3' { Initialize-Pipelines $s; Read-Host 'Enter to go back to the menu' | Out-Null }
        '4' { Backup-Saves; Read-Host 'Enter to go back to the menu' | Out-Null }
        '5' { $s.game_exe = ''; Initialize-Game $s }
        '6' { Set-Dlss $s; Read-Host 'Enter to go back to the menu' | Out-Null }
        'Q' { exit 0 }
    }
}
