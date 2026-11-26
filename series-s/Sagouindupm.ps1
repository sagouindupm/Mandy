# Sagouindupm | VIctory : lanceur pour débutant (profil Series S, cartes 8 Go).
# - Premier lancement : demande où est le jeu, écrit la configuration.
# - Prépare les shaders à l'avance si un dossier de shaders extraits est indiqué ;
#   sinon, tant qu'ils ne sont pas prêts, le jeu attend un shader au lieu d'afficher du noir.
# - Sauvegarde les parties en un clic.
$ErrorActionPreference = 'Stop'
$Root   = Split-Path -Parent $PSScriptRoot
$Exe    = Join-Path $Root 'target\release\victory.exe'
$Home_  = Join-Path $env:LOCALAPPDATA 'Sagouindupm'
$Conf   = Join-Path $Home_ 'lanceur.json'
$Toml   = Join-Path $Home_ 'victory.toml'
$Logs   = Join-Path $Home_ 'journaux'
New-Item -ItemType Directory -Force $Home_, $Logs | Out-Null

function Show-Title {
    Clear-Host
    $Host.UI.RawUI.WindowTitle = 'Sagouindupm | VIctory'
    Write-Host ''
    Write-Host '   ███  Sagouindupm | VIctory  ███' -ForegroundColor Magenta
    Write-Host '   Profil Series S - optimisé pour les petites configs' -ForegroundColor DarkMagenta
    Write-Host ''
}

function Get-Settings {
    if (Test-Path $Conf) { return Get-Content $Conf -Raw | ConvertFrom-Json }
    [pscustomobject]@{ game_exe = ''; shaders_dir = ''; shaders_ready = $false; parties = 0 }
}
function Save-Settings($s) { $s | ConvertTo-Json | Set-Content -Encoding utf8 $Conf }

function Initialize-Game($s) {
    while (-not ($s.game_exe -and (Test-Path $s.game_exe))) {
        Write-Host "Premier lancement : où est le jeu ?" -ForegroundColor Yellow
        Write-Host "Glissez ici le fichier game_scarlett_master.exe de VOTRE copie du jeu, puis Entrée."
        $p = (Read-Host 'Chemin').Trim('"', ' ')
        if ($p -and (Test-Path $p) -and $p.EndsWith('.exe')) { $s.game_exe = $p } else { Write-Host 'Fichier introuvable, réessayez.' -ForegroundColor Red }
    }
    $dir = (Split-Path -Parent $s.game_exe) -replace "'", "''"
    "[paths]`ngame_dir = '$dir'`ntest_titles = []`n" | Set-Content -Encoding utf8 $Toml
    Save-Settings $s
}

# Réglages Series S pour carte 8 Go (les mêmes que series-s\launch_series_s.bat).
$SeriesS = @{
    VICTORY_CARD_ROOM_FOR_TARGETS_MB = '1536'; VICTORY_CARD_BUFFERS_MIN_MB = '1'; VICTORY_CARD_BUFFERS_MB = '512'
    VICTORY_SHADER_LOOP_LIMIT = '256'; VICTORY_TEXTURE_DROP_MIPS = '1'; VICTORY_HEAP_SYNC_TABLES = '1'
    VICTORY_TEXTURE_STAGE_MS = '180'; VICTORY_TEXTURE_CATCHUP = '1'
    VICTORY_HEAP_USAGE = '1'; VICTORY_TEXTURE_SPILL = '1'; VICTORY_LEVELS_ON_DEMAND = '1'
    VICTORY_ASYNC_SUBMIT = '1'; VICTORY_QUEUE2 = '1'; VICTORY_COPY_PLAIN = '1'; VICTORY_FAST_CORES = '1'
    VICTORY_AMBIENT = '0.6'; VICTORY_TEXTURE_BACK = '1'
}
function Set-GameEnv {
    $sdk = Get-ChildItem 'C:\Program Files (x86)\Windows Kits\10\bin\*\x64\dxil.dll' -ErrorAction SilentlyContinue | Sort-Object FullName | Select-Object -Last 1
    if ($sdk) { $env:PATH = "$($sdk.DirectoryName);$env:PATH" }
    $env:VICTORY_CONFIG = $Toml
    foreach ($k in $SeriesS.Keys) { Set-Item "env:$k" $SeriesS[$k] }
}

function Start-Game($s) {
    Set-GameEnv
    if (-not $s.shaders_ready) {
        # Pas de shaders préparés : le jeu attend un shader (jusqu'à 3 s par image) plutôt que
        # de sauter le dessin. Quelques ralentissements au lieu d'écrans noirs.
        $env:VICTORY_PIPELINE_WAIT_MS = '3000'
        Write-Host 'Mode première partie : les shaders se préparent pendant que vous jouez.' -ForegroundColor Yellow
        Write-Host 'Attendez-vous à quelques ralentissements les premières minutes, pas à des écrans noirs.'
    }
    $log = Join-Path $Logs ("partie_{0:yyyy-MM-dd_HHmm}.log" -f (Get-Date))
    Write-Host "`nLancement... (journal : $log)" -ForegroundColor Magenta
    Write-Host 'Clic dans la fenêtre = capturer la souris, Échap = la libérer, G = commandes à pied / véhicule.'
    & $Exe run $s.game_exe --console series-s --execute --unimplemented zero *> $log
    $s.parties++; Save-Settings $s
    Write-Host "`nPartie terminée (code $LASTEXITCODE)."
}

function Initialize-Shaders($s) {
    Write-Host 'Préparer les shaders à l''avance évite les écrans noirs et les à-coups.' -ForegroundColor Yellow
    Write-Host 'Il faut un dossier de shaders extraits de VOTRE copie du jeu (fichiers .dxbc).'
    if (-not ($s.shaders_dir -and (Test-Path $s.shaders_dir))) {
        $p = (Read-Host 'Dossier des shaders (vide = annuler)').Trim('"', ' ')
        if (-not $p -or -not (Test-Path $p)) { Write-Host 'Annulé.'; return }
        $s.shaders_dir = $p; Save-Settings $s
    }
    Set-GameEnv
    $env:VICTORY_HOST_WAVE = '32'; $env:VICTORY_HOST_ROVS = '1'; $env:VICTORY_VIEW_LIMIT = '999999'
    Write-Host "`nTraduction de tous les shaders à l'avance (20 à 40 minutes, le PC sera très occupé)..." -ForegroundColor Magenta
    Write-Host "Vous pouvez l'arrêter (Ctrl+C) et la reprendre plus tard : ce qui est fait reste en cache."
    & $Exe translate $s.shaders_dir --warm all --jobs ([Math]::Max(2, [Environment]::ProcessorCount - 2)) | Tee-Object -FilePath (Join-Path $Logs 'shaders_traduction.txt') | Where-Object { $_ -match 'warm-up' }
    if ($LASTEXITCODE -eq 0) { $s.shaders_ready = $true; Save-Settings $s; Write-Host 'Shaders prêts !' -ForegroundColor Green }
    else { Write-Host "La traduction s'est arrêtée (code $LASTEXITCODE), voir $Logs." -ForegroundColor Red }
}

function Backup-Saves {
    $data = Join-Path $env:LOCALAPPDATA 'victory\data'
    if (-not (Test-Path $data)) { Write-Host 'Aucune sauvegarde trouvée pour le moment.'; return }
    $dst = Join-Path $Home_ ("sauvegardes\{0:yyyy-MM-dd_HHmmss}" -f (Get-Date))
    Copy-Item -Recurse $data $dst
    Write-Host "Parties copiées dans $dst" -ForegroundColor Green
}

if (-not (Test-Path $Exe)) {
    Show-Title
    Write-Host "victory.exe introuvable ($Exe). Compilez d'abord : cargo build --release" -ForegroundColor Red
    Read-Host 'Entrée pour quitter'; exit 1
}
$s = Get-Settings
Show-Title
Initialize-Game $s
while ($true) {
    Show-Title
    $state = if ($s.shaders_ready) { 'prêts' } else { 'pas encore préparés' }
    Write-Host "  Shaders : $state   |   Parties lancées : $($s.parties)`n"
    Write-Host '  [1] Jouer'
    Write-Host '  [2] Préparer les shaders à l''avance (recommandé la première fois)'
    Write-Host '  [3] Copier mes sauvegardes en lieu sûr'
    Write-Host '  [4] Changer l''emplacement du jeu'
    Write-Host '  [Q] Quitter'
    switch ((Read-Host "`nChoix").Trim().ToUpper()) {
        '1' { Start-Game $s; Read-Host 'Entrée pour revenir au menu' | Out-Null }
        '2' { Initialize-Shaders $s; Read-Host 'Entrée pour revenir au menu' | Out-Null }
        '3' { Backup-Saves; Read-Host 'Entrée pour revenir au menu' | Out-Null }
        '4' { $s.game_exe = ''; Initialize-Game $s }
        'Q' { exit 0 }
    }
}
