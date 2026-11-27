# Sagouindupm | VIctory : la seule source des réglages Series S et des chemins des lanceurs.
# Chargé (dot-source) par le lanceur console Sagouindupm.ps1 et le lanceur graphique
# Sagouindupm-Launcher.ps1 : un réglage changé ici vaut pour les deux.
# Les scripts de mesure (scripts/bench) appellent victory.exe directement et ne lisent pas ce fichier.

$Root   = Split-Path -Parent $PSScriptRoot
$Exe    = Join-Path $Root 'target\release\victory.exe'
$Home_  = Join-Path $env:LOCALAPPDATA 'Sagouindupm'
$Conf   = Join-Path $Home_ 'lanceur.json'
$Toml   = Join-Path $Home_ 'victory.toml'
$Logs   = Join-Path $Home_ 'journaux'
New-Item -ItemType Directory -Force $Home_, $Logs | Out-Null

function Get-Settings {
    if (Test-Path $Conf) { return Get-Content $Conf -Raw | ConvertFrom-Json }
    [pscustomobject]@{ game_exe = ''; shaders_dir = ''; shaders_ready = $false; parties = 0 }
}
# Réglages ajoutés après la première version du lanceur : présents même dans un ancien lanceur.json.
#   dlss_dir, dlss_mode : DLSS (dossier de Streamline, mode).
#   pipelines_parties   : nombre de parties lancées au dernier « victory warm --used » réussi (-1 : jamais).
#   nv_cache_ok         : le joueur a dit avoir réglé le cache de shaders NVIDIA sur « Illimitée ».
#   ancien_plan         : mode d'alimentation actif avant que le lanceur graphique le change (pour revenir).
function Complete-Settings($s) {
    $defaults = [ordered]@{ dlss_dir = ''; dlss_mode = ''; pipelines_parties = -1; nv_cache_ok = $false; ancien_plan = '' }
    foreach ($k in $defaults.Keys) {
        if (-not ($s.PSObject.Properties.Name -contains $k)) { $s | Add-Member -NotePropertyName $k -NotePropertyValue $defaults[$k] }
    }
    $s
}
function Save-Settings($s) { $s | ConvertTo-Json | Set-Content -Encoding utf8 $Conf }

# victory.toml : le dossier du jeu, lu par victory.exe (VICTORY_CONFIG).
function Write-VictoryToml($s) {
    $dir = (Split-Path -Parent $s.game_exe) -replace "'", "''"
    "[paths]`ngame_dir = '$dir'`ntest_titles = []`n" | Set-Content -Encoding utf8 $Toml
    Save-Settings $s
}

# Réglages Series S (base trustno1-coder/VIctory-NVIDIA réglée pour RTX 3060 par Maykzera), mesurés ici sur
# RTX 3050 8 Go + i5-12400F : ~84 ms par image (11,8 fps) dans la cinématique du bateau, eau et visages fidèles.
function Get-SeriesSSettings {
    @{
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
        # Chaque partie note les pipelines créées, pour « Construire les pipelines » (victory warm --used)
        VICTORY_PIPELINE_BOOK = '1'
    }
}

# Pipelines sautées : les ~1 000 passes de résolution du ray tracing (pas de structure de rayons sur NVIDIA),
# + la brume de chaleur 731a39171b568298 (foxhound : elle étale des blocs de pixels, aussi sur l'eau) :
# 84 ms contre 93 sur RTX 3050, image propre.
function Get-SkipPipelines {
    $rt = Join-Path $Root 'scripts\rt_off_hashes.txt'
    $skip = @('731a39171b568298')
    if (Test-Path $rt) { $skip = @((Get-Content $rt -Raw).Trim()) + $skip }
    $skip -join ','
}

# Mémoire vive totale en octets (0 si inconnue).
function Get-TotalRam {
    try { (Get-CimInstance Win32_ComputerSystem -ErrorAction Stop).TotalPhysicalMemory } catch { 0 }
}

# Prépare l'environnement de victory.exe (dans ce processus : les programmes lancés ensuite en héritent).
function Set-GameEnv($s) {
    $sdk = Get-ChildItem 'C:\Program Files (x86)\Windows Kits\10\bin\*\x64\dxil.dll' -ErrorAction SilentlyContinue | Sort-Object FullName | Select-Object -Last 1
    if ($sdk -and -not ($env:PATH -like "$($sdk.DirectoryName);*")) { $env:PATH = "$($sdk.DirectoryName);$env:PATH" }
    $env:VICTORY_CONFIG = $Toml
    $seriesS = Get-SeriesSSettings
    foreach ($k in $seriesS.Keys) { Set-Item "env:$k" $seriesS[$k] }
    $env:VICTORY_SKIP_PIPELINES = Get-SkipPipelines
    # PC à moins de 24 Go : fenêtres de mémoire du jeu dans la mémoire de la carte (évite le plantage 0x8007000E).
    $ram = Get-TotalRam
    if ($ram -gt 0 -and $ram -lt 24GB) { $env:VICTORY_WINDOWS_BAR_MB = '3072' }
    # DLSS (cartes RTX) à la place de l'upscaler du jeu, si le dossier de Streamline est donné.
    # Mesuré sur RTX 3050 : ultra performance 90 ms (image propre), performance 102 ms, qualité 176 ms.
    Remove-Item env:VICTORY_DLSS_DIR, env:VICTORY_DLSS_MODE -ErrorAction SilentlyContinue
    if ($s.dlss_mode -and $s.dlss_mode -ne 'off' -and $s.dlss_dir -and (Test-Path (Join-Path $s.dlss_dir 'sl.interposer.dll'))) {
        $env:VICTORY_DLSS_DIR = $s.dlss_dir; $env:VICTORY_DLSS_MODE = $s.dlss_mode
        Remove-Item env:VICTORY_RENDER_SCALE, env:VICTORY_OWN_UPSCALER -ErrorAction SilentlyContinue
    }
}

# Les faits de la carte pour la traduction des shaders (NVIDIA : vagues de 32, 1 000 000 de vues) :
# sans eux le cache ne sert à rien en jeu.
function Get-ShaderHostEnv {
    @{ VICTORY_HOST_WAVE = '32'; VICTORY_HOST_ROVS = '1'; VICTORY_VIEW_LIMIT = '999999' }
}

# Threads pour « victory shaders » : tous les cœurs logiques sauf deux (au moins 2).
function Get-ShaderJobs { [Math]::Max(2, [Environment]::ProcessorCount - 2) }
