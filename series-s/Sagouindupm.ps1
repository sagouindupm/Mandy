# Sagouindupm | VIctory : lanceur pour débutant (profil Series S, cartes NVIDIA 6-12 Go, processeur 6 cœurs).
# 1. Premier lancement : demande où est le jeu, écrit la configuration.
# 2. [2] Prépare les shaders : traduit tous les shaders du jeu une fois (15-30 min).
# 3. [3] Construit les pipelines hors jeu, dès avant la première partie : importe la liste publiée
#    (series-s\pipelines\series-s-rtx.txt, sans code du jeu) avec les shaders de VOTRE copie, puis victory warm.
# 4. [1] Jouer : chaque partie note aussi les pipelines que le jeu crée (VICTORY_PIPELINE_BOOK) ; [3] les construit.
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
# Réglages ajoutés après la première version du lanceur : présents même dans un ancien lanceur.json.
function Complete-Settings($s) {
    foreach ($k in 'dlss_dir', 'dlss_mode') {
        if (-not ($s.PSObject.Properties.Name -contains $k)) { $s | Add-Member -NotePropertyName $k -NotePropertyValue '' }
    }
    $s
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
# RTX 3050 8 Go + i5-12400F : ~84 ms par image (11,8 fps) dans la cinématique du bateau, eau et visages fidèles.
$SeriesS = @{
    # Série rapide (avatarmanz/trustno1)
    VICTORY_TEXTURE_NO_WAIT = '1'; VICTORY_RELAY = '2'; VICTORY_CARD_STEPS = '2'
    VICTORY_WIDE_BACK = '1'; VICTORY_CARD_REHASH_RUNS = '1'; VICTORY_JOIN_COPIES = '1'
    VICTORY_INDIRECT_WINDOW = '1'; VICTORY_RELAY_COPY = '1'; VICTORY_INDIRECT_BATCH = '2'
    VICTORY_MAP_THREADS = '1'; VICTORY_TITLE_PRIORITIES = '0'; VICTORY_CARD_AWAKE = '1'
    VICTORY_SHOW_AFTER = '1'; VICTORY_THIN = '1'; VICTORY_UPLOAD_AHEAD = '1'
    # Processeur 6 cœurs / 12 fils (Maykzera) : moins de fils auxiliaires, 16 soumissions préparées ensemble ;
    # 4 fils de traduction des listes et pas 5 (Maykzera, 2 mesures ; ici 87 ms contre 93, pire image 178 ms)
    VICTORY_THIN_THREADS = '4'; VICTORY_UNDER_THREADS = '3'; VICTORY_VIEW_THREADS = '3'
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
    # + la brume de chaleur 731a39171b568298 (foxhound : elle étale des blocs de pixels, aussi sur l'eau) :
    #   84 ms contre 93 sur RTX 3050, image propre.
    # + les programmes que le repli LDS rend traduisibles mais qui coûtent ~30 ms sans les arbres
    #   (volumétrie de l'eau 5dc3/6f22, sous l'eau 926c, transmission 1f1f, 21f7, 7c9a) : laissés de côté
    #   comme avant ; les arbres (55679a05, 89d08e56) restent. 97 ms contre 106 avec tout.
    $skip = @('731a39171b568298', '5dc3b25986622949', '6f225b641c5b0708', '926c46bab3b86bd6', '1f1fbd86759b3da2', '21f7ad7863620328', '7c9a6a83826cdfac')
    if (Test-Path $rt) { $skip = @((Get-Content $rt -Raw).Trim()) + $skip }
    $env:VICTORY_SKIP_PIPELINES = $skip -join ','
    # PC à moins de 24 Go : fenêtres de mémoire du jeu dans la mémoire de la carte (évite le plantage 0x8007000E).
    $ram = try { (Get-CimInstance Win32_ComputerSystem -ErrorAction Stop).TotalPhysicalMemory } catch { 0 }
    if ($ram -gt 0 -and $ram -lt 24GB) { $env:VICTORY_WINDOWS_BAR_MB = '3072' }
    # DLSS (cartes RTX) à la place de l'upscaler du jeu, si le dossier de Streamline est donné ([6]).
    # Mesuré sur RTX 3050 : ultra performance 90 ms (image propre), performance 102 ms, qualité 176 ms.
    Remove-Item env:VICTORY_DLSS_DIR, env:VICTORY_DLSS_MODE -ErrorAction SilentlyContinue
    if ($s.dlss_mode -and $s.dlss_mode -ne 'off' -and $s.dlss_dir -and (Test-Path (Join-Path $s.dlss_dir 'sl.interposer.dll'))) {
        $env:VICTORY_DLSS_DIR = $s.dlss_dir; $env:VICTORY_DLSS_MODE = $s.dlss_mode
        Remove-Item env:VICTORY_RENDER_SCALE, env:VICTORY_OWN_UPSCALER -ErrorAction SilentlyContinue
    }
}

function Set-Dlss($s) {
    Write-Host 'DLSS (cartes NVIDIA RTX) : il faut les DLL de Streamline de NVIDIA (sl.interposer.dll, sl.common.dll,' -ForegroundColor Yellow
    Write-Host 'sl.dlss.dll, nvngx_dlss.dll), à télécharger vous-même : https://github.com/NVIDIA-RTX/Streamline/releases'
    Write-Host '(dossier bin\x64 de l''archive). Elles ne sont pas fournies ici (licence NVIDIA).'
    $p = (Read-Host "Dossier qui contient sl.interposer.dll (vide = garder '$($s.dlss_dir)')").Trim('"', ' ')
    if ($p) {
        if (Test-Path (Join-Path $p 'sl.interposer.dll')) { $s.dlss_dir = $p } else { Write-Host 'sl.interposer.dll introuvable dans ce dossier.' -ForegroundColor Red; return }
    }
    Write-Host '  [1] Ultra performance (le plus rapide, conseillé sur 6-8 Go)'
    Write-Host '  [2] Performance'
    Write-Host '  [3] Qualité (le plus beau, plus lent)'
    Write-Host '  [4] Désactivé (upscaler du jeu)'
    $s.dlss_mode = switch ((Read-Host 'Mode').Trim()) { '1' { 'ultra' } '2' { 'performance' } '3' { 'quality' } default { 'off' } }
    Save-Settings $s
    Write-Host "DLSS : $($s.dlss_mode)" -ForegroundColor Green
}

function Start-Game($s) {
    Set-GameEnv
    if (-not $s.shaders_ready) {
        Write-Host 'Shaders pas encore préparés : les premières minutes seront lentes. Conseil : [2] puis [3] d''abord.' -ForegroundColor Yellow
    }
    $log = Join-Path $Logs ("partie_{0:yyyy-MM-dd_HHmm}.log" -f (Get-Date))
    Write-Host "`nLancement... (journal : $log)" -ForegroundColor Magenta
    Write-Host 'Après une partie, [3] construit aussi les pipelines notées que la liste publiée n''avait pas.'
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
    # La liste publiée d'abord : la traduction garde au passage les shaders de vos fichiers qu'elle nomme ([3]).
    $list = Join-Path $Root 'series-s\pipelines\series-s-rtx.txt'
    if (Test-Path $list) { & $Exe pipelines import $list | Tee-Object -FilePath (Join-Path $Logs 'pipelines_import.txt') }
    Write-Host "`nTraduction de tous les shaders (15 à 30 minutes, le PC sera très occupé)..." -ForegroundColor Magenta
    & $Exe shaders $s.shaders_dir --jobs ([Math]::Max(2, [Environment]::ProcessorCount - 2)) | Tee-Object -FilePath (Join-Path $Logs 'shaders_traduction.txt') | Select-Object -Last 3
    if ($LASTEXITCODE -eq 0) { $s.shaders_ready = $true; Save-Settings $s; Write-Host 'Shaders prêts !' -ForegroundColor Green }
    else { Write-Host "La traduction s'est arrêtée (code $LASTEXITCODE), voir $Logs." -ForegroundColor Red }
}

function Initialize-Pipelines($s) {
    Set-GameEnv
    # La liste publiée : les pipelines dont le jeu se sert jusqu'après la mission du bateau (clés, empreintes des
    # shaders, états ; aucun code du jeu). Les shaders eux-mêmes viennent de VOTRE dossier de shaders : gardés
    # par [2] ; sinon --shaders les y cherche (~7 min ; rien à faire s'ils sont déjà là).
    $list = Join-Path $Root 'series-s\pipelines\series-s-rtx.txt'
    if (Test-Path $list) {
        Write-Host "`nImport de la liste des pipelines publiée..." -ForegroundColor Magenta
        $import = @('pipelines', 'import', $list)
        if ($s.shaders_dir -and (Test-Path $s.shaders_dir)) { $import += @('--shaders', $s.shaders_dir) }
        else { Write-Host 'Dossier de shaders inconnu : faites [2] d''abord, sinon seules les pipelines de vos parties seront construites.' -ForegroundColor Yellow }
        & $Exe @import | Tee-Object -FilePath (Join-Path $Logs 'pipelines_import.txt')
    }
    Write-Host "`nConstruction des pipelines (liste publiée et vos parties, quelques minutes)..." -ForegroundColor Magenta
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
$s = Complete-Settings (Get-Settings)
Show-Title
Initialize-Game $s
while ($true) {
    Show-Title
    $state = if ($s.shaders_ready) { 'prêts' } else { 'pas encore préparés' }
    $dlss = if ($s.dlss_mode -and $s.dlss_mode -ne 'off') { $s.dlss_mode } else { 'désactivé' }
    Write-Host "  Shaders : $state   |   DLSS : $dlss   |   Parties lancées : $($s.parties)`n"
    Write-Host '  [1] Jouer'
    Write-Host '  [2] Préparer les shaders à l''avance (une fois, recommandé)'
    Write-Host '  [3] Construire les pipelines (après [2], avant la première partie ; à refaire après une partie)'
    Write-Host '  [4] Copier mes sauvegardes en lieu sûr'
    Write-Host '  [5] Changer l''emplacement du jeu'
    Write-Host '  [6] DLSS (cartes NVIDIA RTX)'
    Write-Host '  [Q] Quitter'
    switch ((Read-Host "`nChoix").Trim().ToUpper()) {
        '1' { Start-Game $s; Read-Host 'Entrée pour revenir au menu' | Out-Null }
        '2' { Initialize-Shaders $s; Read-Host 'Entrée pour revenir au menu' | Out-Null }
        '3' { Initialize-Pipelines $s; Read-Host 'Entrée pour revenir au menu' | Out-Null }
        '4' { Backup-Saves; Read-Host 'Entrée pour revenir au menu' | Out-Null }
        '5' { $s.game_exe = ''; Initialize-Game $s }
        '6' { Set-Dlss $s; Read-Host 'Entrée pour revenir au menu' | Out-Null }
        'Q' { exit 0 }
    }
}
