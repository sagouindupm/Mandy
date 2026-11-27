# Sagouindupm | VIctory : chemins et lanceur.json du lanceur console Sagouindupm.ps1, et application des
# réglages Series S. Les réglages eux-mêmes sont dans reglages.json, la seule source, que le lanceur graphique
# (launcher/) lit aussi : un réglage changé là vaut pour les deux.
# Les scripts de mesure (scripts/bench) appellent victory.exe directement et ne lisent pas ces fichiers.

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

# Réglages Series S, pipelines sautées, règle des 24 Go, DLSS : series-s\reglages.json (lu aussi par le lanceur
# graphique). Ce fichier ne fait que l'appliquer.
$Reglages = Get-Content (Join-Path $PSScriptRoot 'reglages.json') -Raw -Encoding UTF8 | ConvertFrom-Json

function Get-SeriesSSettings {
    $h = @{}
    foreach ($g in $Reglages.groupes) { foreach ($p in $g.env.PSObject.Properties) { $h[$p.Name] = [string]$p.Value } }
    $h
}

# Pipelines sautées : le fichier des passes du ray tracing (relatif à la racine du dépôt) puis les hachages en plus.
function Get-SkipPipelines {
    $rt = Join-Path $Root $Reglages.pipelines_sautees.fichier
    $skip = @($Reglages.pipelines_sautees.en_plus)
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
    # Petite mémoire vive : voir « petite_ram » dans reglages.json.
    $ram = Get-TotalRam
    if ($ram -gt 0 -and $ram -lt ([double]$Reglages.petite_ram.moins_de_go * 1GB)) {
        foreach ($p in $Reglages.petite_ram.env.PSObject.Properties) { Set-Item "env:$($p.Name)" ([string]$p.Value) }
    }
    # DLSS : voir « dlss » dans reglages.json.
    $d = $Reglages.dlss
    Remove-Item "env:$($d.variable_dossier)", "env:$($d.variable_mode)" -ErrorAction SilentlyContinue
    if ($s.dlss_mode -and $s.dlss_mode -ne 'off' -and $s.dlss_dir -and (Test-Path (Join-Path $s.dlss_dir $d.fichier_requis))) {
        Set-Item "env:$($d.variable_dossier)" $s.dlss_dir; Set-Item "env:$($d.variable_mode)" $s.dlss_mode
        foreach ($v in $d.retire) { Remove-Item "env:$v" -ErrorAction SilentlyContinue }
    }
}

# Les faits de la carte pour la traduction des shaders (NVIDIA) : voir « shaders » dans reglages.json.
function Get-ShaderHostEnv {
    $h = @{}
    foreach ($p in $Reglages.shaders.env_nvidia.PSObject.Properties) { $h[$p.Name] = [string]$p.Value }
    $h
}

# Threads pour « victory shaders » : tous les cœurs logiques sauf deux (au moins 2).
function Get-ShaderJobs { [Math]::Max([int]$Reglages.shaders.threads_min, [Environment]::ProcessorCount - [int]$Reglages.shaders.threads_laisses_libres) }
