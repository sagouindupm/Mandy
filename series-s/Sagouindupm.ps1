# Sagouindupm | VIctory : lanceur pour débutant (profil Series S, cartes NVIDIA 6-12 Go, processeur 6 cœurs).
# 1. Premier lancement : demande où est le jeu, écrit la configuration.
# 2. [2] Prépare les shaders : traduit tous les shaders du jeu une fois (15-30 min).
# 3. [1] Jouer : chaque partie note les pipelines que le jeu crée (VICTORY_PIPELINE_BOOK).
# 4. [3] Construit ces pipelines hors jeu (victory warm) : plus d'à-coups ni de dessins manquants ensuite.
# Pas 'Stop' : sous Windows PowerShell 5.1, chaque ligne que le jeu écrit sur sa sortie d'erreur
# devient une erreur, et le lanceur s'arrêtait à la première ligne du journal.
$ErrorActionPreference = 'Continue'
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

# Réglages Series S (base trustno1-coder/VIctory-NVIDIA réglée pour RTX 3060 par Maykzera), mesurés ici sur
# RTX 3050 8 Go + i5-12400F : ~103 ms par image (9,7 fps) dans la cinématique du bateau, eau et visages fidèles.
$SeriesS = @{
    # Série rapide (avatarmanz/trustno1)
    VICTORY_TEXTURE_NO_WAIT = '1'; VICTORY_RELAY = '2'; VICTORY_CARD_STEPS = '2'
    VICTORY_WIDE_BACK = '1'; VICTORY_CARD_REHASH_RUNS = '1'; VICTORY_JOIN_COPIES = '1'
    VICTORY_INDIRECT_WINDOW = '1'; VICTORY_RELAY_COPY = '1'; VICTORY_INDIRECT_BATCH = '2'
    VICTORY_MAP_THREADS = '1'; VICTORY_TITLE_PRIORITIES = '0'; VICTORY_CARD_AWAKE = '1'
    VICTORY_SHOW_AFTER = '1'; VICTORY_THIN = '1'; VICTORY_UPLOAD_AHEAD = '1'
    # Processeur 6 cœurs / 12 fils (Maykzera) : moins de fils auxiliaires, 16 soumissions préparées ensemble
    VICTORY_THIN_THREADS = '5'; VICTORY_UNDER_THREADS = '3'; VICTORY_VIEW_THREADS = '3'
    VICTORY_PIPELINE_WORKERS = '4'; VICTORY_PIPELINE_LOWEST = '1'
    VICTORY_JOIN_PARTS = '16'; VICTORY_JOIN_LISTS = '96'
    # Une pipeline pas prête : 150 ms d'attente au plus (10 000 gelait l'image 12 s, avatarmanz #143)
    VICTORY_PIPELINE_WAIT_MS = '150'
    # Image : lumière de remplacement du ray tracing, NaN effacés, 540p + upscaler du jeu, interface Series S
    VICTORY_AMBIENT = '0.8'; VICTORY_AMBIENT_AIR = '1.5'; VICTORY_AMBIENT_MOST = '2'
    VICTORY_NAN_SCRUB = '1'; VICTORY_BVH_MISS = '1'; VICTORY_DYNAMIC_SLOTS = '524288'
    VICTORY_RENDER_SCALE = '0.5'; VICTORY_OWN_UPSCALER = '1'; VICTORY_COMPOSE = '1'
    VICTORY_CARD_BUFFERS_MIN_MB = '1'; VICTORY_CARD_BUFFERS_MB = '1536'
    # Chaque partie note les pipelines créées, pour [3]
    VICTORY_PIPELINE_BOOK = '1'
}
function Set-GameEnv {
    $sdk = Get-ChildItem 'C:\Program Files (x86)\Windows Kits\10\bin\*\x64\dxil.dll' -ErrorAction SilentlyContinue | Sort-Object FullName | Select-Object -Last 1
    if ($sdk) { $env:PATH = "$($sdk.DirectoryName);$env:PATH" }
    $env:VICTORY_CONFIG = $Toml
    foreach ($k in $SeriesS.Keys) { Set-Item "env:$k" $SeriesS[$k] }
    # Ray tracing : les ~1 000 passes de résolution sont sautées (pas de structure de rayons sur NVIDIA).
    $rt = Join-Path $Root 'scripts\rt_off_hashes.txt'
    if (Test-Path $rt) { $env:VICTORY_SKIP_PIPELINES = (Get-Content $rt -Raw).Trim() }
    # PC à moins de 24 Go : fenêtres de mémoire du jeu dans la mémoire de la carte (évite le plantage 0x8007000E).
    $ram = try { (Get-CimInstance Win32_ComputerSystem -ErrorAction Stop).TotalPhysicalMemory } catch { 0 }
    if ($ram -gt 0 -and $ram -lt 24GB) { $env:VICTORY_WINDOWS_BAR_MB = '3072' }
}

function Start-Game($s) {
    Set-GameEnv
    if (-not $s.shaders_ready) {
        Write-Host 'Shaders pas encore préparés : les premières minutes seront lentes. Conseil : [2] d''abord.' -ForegroundColor Yellow
    }
    $log = Join-Path $Logs ("partie_{0:yyyy-MM-dd_HHmm}.log" -f (Get-Date))
    Write-Host "`nLancement... (journal : $log)" -ForegroundColor Magenta
    Write-Host 'Après une première partie, [3] construit les pipelines notées : les suivantes sont plus fluides.'
    # Par cmd : PowerShell 5.1 ne transforme pas les lignes du journal en erreurs.
    & cmd.exe /c "`"$Exe`" run `"$($s.game_exe)`" --console series-s --execute > `"$log`" 2>&1"
    $s.parties++; Save-Settings $s
    Write-Host "`nPartie terminée (code $LASTEXITCODE)."
}

function Initialize-Shaders($s) {
    Write-Host 'Préparer les shaders à l''avance évite les écrans noirs et les à-coups.' -ForegroundColor Yellow
    Write-Host 'Il faut un dossier de shaders extraits de VOTRE copie du jeu (ou son shaders.rpf).'
    if (-not ($s.shaders_dir -and (Test-Path $s.shaders_dir))) {
        $p = (Read-Host 'Dossier des shaders (vide = annuler)').Trim('"', ' ')
        if (-not $p -or -not (Test-Path $p)) { Write-Host 'Annulé.'; return }
        $s.shaders_dir = $p; Save-Settings $s
    }
    Set-GameEnv
    # Les faits de la carte (NVIDIA : vagues de 32, 1 000 000 de vues) : sans eux le cache ne sert à rien en jeu.
    $env:VICTORY_HOST_WAVE = '32'; $env:VICTORY_HOST_ROVS = '1'; $env:VICTORY_VIEW_LIMIT = '999999'
    Write-Host "`nTraduction de tous les shaders (15 à 30 minutes, le PC sera très occupé)..." -ForegroundColor Magenta
    & $Exe shaders $s.shaders_dir --jobs ([Math]::Max(2, [Environment]::ProcessorCount - 2)) | Tee-Object -FilePath (Join-Path $Logs 'shaders_traduction.txt') | Select-Object -Last 3
    if ($LASTEXITCODE -eq 0) { $s.shaders_ready = $true; Save-Settings $s; Write-Host 'Shaders prêts !' -ForegroundColor Green }
    else { Write-Host "La traduction s'est arrêtée (code $LASTEXITCODE), voir $Logs." -ForegroundColor Red }
}

function Initialize-Pipelines {
    Set-GameEnv
    Write-Host "`nConstruction des pipelines notées pendant vos parties (quelques minutes)..." -ForegroundColor Magenta
    & $Exe warm --used | Tee-Object -FilePath (Join-Path $Logs 'pipelines_warm.txt') | Select-Object -Last 2
    Write-Host 'Pipelines prêtes : la prochaine partie démarre sans à-coups.' -ForegroundColor Green
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
    Write-Host "victory.exe introuvable ($Exe). Compilez d'abord : cargo build --release --locked -p victory" -ForegroundColor Red
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
    Write-Host '  [2] Préparer les shaders à l''avance (une fois, recommandé)'
    Write-Host '  [3] Construire les pipelines (après une première partie)'
    Write-Host '  [4] Copier mes sauvegardes en lieu sûr'
    Write-Host '  [5] Changer l''emplacement du jeu'
    Write-Host '  [Q] Quitter'
    switch ((Read-Host "`nChoix").Trim().ToUpper()) {
        '1' { Start-Game $s; Read-Host 'Entrée pour revenir au menu' | Out-Null }
        '2' { Initialize-Shaders $s; Read-Host 'Entrée pour revenir au menu' | Out-Null }
        '3' { Initialize-Pipelines; Read-Host 'Entrée pour revenir au menu' | Out-Null }
        '4' { Backup-Saves; Read-Host 'Entrée pour revenir au menu' | Out-Null }
        '5' { $s.game_exe = ''; Initialize-Game $s }
        'Q' { exit 0 }
    }
}
