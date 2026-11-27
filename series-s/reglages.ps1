# Sagouindupm | VIctory : chemins et lanceur.json du lanceur console Sagouindupm.ps1, et application des
# réglages Series S. Les réglages eux-mêmes sont dans reglages.json, la seule source, que le lanceur graphique
# (launcher/) lit aussi : un réglage changé là vaut pour les deux.
# Les scripts de mesure (scripts/bench) appellent victory.exe directement et ne lisent pas ces fichiers.

$Root   = Split-Path -Parent $PSScriptRoot
# La version téléchargée (scripts/package-release.ps1) a victory.exe dans engine\ ; un dépôt, dans target\release.
$Exe    = Join-Path $Root 'engine\victory.exe'
if (-not (Test-Path $Exe)) { $Exe = Join-Path $Root 'target\release\victory.exe' }
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
#   pipelines_liste     : repère « nom:taille » de la liste partagée construite par ce « warm » ('' : aucune).
#   nv_cache_ok         : le joueur a dit avoir réglé le cache de shaders NVIDIA sur « Illimitée ».
#   ancien_plan         : mode d'alimentation actif avant que le lanceur graphique le change (pour revenir).
function Complete-Settings($s) {
    $defaults = [ordered]@{ dlss_dir = ''; dlss_mode = ''; pipelines_parties = -1; pipelines_liste = ''; nv_cache_ok = $false; ancien_plan = '' }
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

# Réglages Series S, pipelines sautées, profil du matériel, DLSS : series-s\reglages.json (lu aussi par le lanceur
# graphique). Ce fichier ne fait que l'appliquer.
$Reglages = Get-Content (Join-Path $PSScriptRoot 'reglages.json') -Raw -Encoding UTF8 | ConvertFrom-Json

function Get-SeriesSSettings {
    $h = [ordered]@{}
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

# La liste partagée des pipelines (« pipelines_partagees » dans reglages.json), $null si absente.
function Get-SharedPipelineList {
    $f = Join-Path $Root $Reglages.pipelines_partagees.fichier
    if (Test-Path $f) { $f } else { $null }
}

# Son repère « nom:taille », pour savoir quelle liste a été construite (le même que le lanceur graphique).
function Get-SharedPipelineMark {
    $f = Get-SharedPipelineList
    if ($f) { $i = Get-Item $f; "$($i.Name):$($i.Length)" } else { '' }
}

# Mémoire vive totale en octets (0 si inconnue).
function Get-TotalRam {
    try { (Get-CimInstance Win32_ComputerSystem -ErrorAction Stop).TotalPhysicalMemory } catch { 0 }
}

# ------------------------------------------------------------------ profil du matériel
# « materiel » dans reglages.json, le même calcul que launcher/src-tauri/src/profile.rs (scripts/check-profils.ps1
# compare les deux).

# La carte graphique : NVIDIA d'abord, sinon celle qui a le plus de mémoire (taille 64 bits du pilote ; AdapterRAM
# s'arrête à 4 Go). Le même choix que le lanceur graphique (sys.rs).
function Get-Gpu {
    $mem = @{}
    Get-ItemProperty 'HKLM:\SYSTEM\CurrentControlSet\Control\Class\{4d36e968-e325-11ce-bfc1-08002be10318}\0*' -ErrorAction SilentlyContinue | ForEach-Object {
        $q = $_.'HardwareInformation.qwMemorySize'
        if ($_.DriverDesc -and $q) { try { $mem[[string]$_.DriverDesc] = [double]$q } catch { } }
    }
    $gpus = @(Get-CimInstance Win32_VideoController -ErrorAction SilentlyContinue | Where-Object { $_.Name -notmatch 'Basic Display|Remote|Virtual|Parsec|Meta Virtual' } | ForEach-Object {
        $v = if ($mem.ContainsKey([string]$_.Name)) { $mem[[string]$_.Name] } else { [double]$_.AdapterRAM }
        [pscustomobject]@{ Name = [string]$_.Name; Vram = $v }
    })
    $gpu = $gpus | Where-Object { $_.Name -match 'NVIDIA' } | Sort-Object Vram -Descending | Select-Object -First 1
    if (-not $gpu) { $gpu = $gpus | Sort-Object Vram -Descending | Select-Object -First 1 }
    if ($gpu) { $gpu } else { [pscustomobject]@{ Name = ''; Vram = [double]0 } }
}

# Ce PC (lu une fois) : mémoire vive, mémoire de la carte, fils du processeur, nom de la carte. 0 = inconnu.
$script:Hardware = $null
function Get-Hardware {
    if (-not $script:Hardware) {
        $gpu = Get-Gpu
        $script:Hardware = [pscustomobject]@{
            RamBytes = [double](Get-TotalRam); VramBytes = [double]$gpu.Vram
            Threads = [Environment]::ProcessorCount; GpuName = $gpu.Name
        }
    }
    $script:Hardware
}

# nvidia_rtx, nvidia, amd, intel_arc, intel_integree ou inconnu, d'après le nom de la carte.
function Get-GpuVendor([string]$Name) {
    $n = $Name.ToUpper()
    if ($n.Contains('NVIDIA') -or $n.Contains('GEFORCE')) { if ($n.Contains('RTX')) { return 'nvidia_rtx' } else { return 'nvidia' } }
    if ($n.Contains('AMD') -or $n.Contains('RADEON')) { return 'amd' }
    if ($n.Contains('INTEL')) {
        # Les cartes Arc dédiées ont un numéro (A380, A770, B580) ; les puces Arc des processeurs, non.
        $card = @($n -split '[^A-Z0-9]' | Where-Object { $_ -cmatch '^[AB][0-9]{3}$' }).Count -gt 0
        if ($n.Contains('ARC') -and $card) { return 'intel_arc' } else { return 'intel_integree' }
    }
    'inconnu'
}

function Format-Env($o) { ($o.PSObject.Properties | ForEach-Object { "$($_.Name -replace '^VICTORY_', '')=$($_.Value)" }) -join ', ' }

# Le profil de ce PC : Env (ce qui change par rapport aux groupes), Reasons, Warnings, Tips, Dlss, ShadersNvidia.
function Get-HardwareProfile($hw) {
    $m = $Reglages.materiel
    $base = Get-SeriesSSettings
    $envs = [ordered]@{}
    $reasons = @(); $warnings = @(); $tips = @()
    $ramGb = $hw.RamBytes / 1GB; $vramGb = $hw.VramBytes / 1GB
    $veryLow = $hw.RamBytes -gt 0 -and $ramGb -lt [double]$m.tres_petite_ram.moins_de_go

    # Processeur : le premier cran sous sa limite ; un cran plus bas sur un PC de 16 Go.
    $cpu = @($m.processeur.crans)
    if ($hw.Threads -gt 0 -and $cpu.Count -gt 0) {
        $found = $cpu.Count - 1
        for ($i = 0; $i -lt $cpu.Count; $i++) { if ($null -eq $cpu[$i].moins_de_fils -or $hw.Threads -lt [int]$cpu[$i].moins_de_fils) { $found = $i; break } }
        $idx = if ($veryLow) { [Math]::Max(0, $found - [int]$m.tres_petite_ram.crans_de_fils_en_moins) } else { $found }
        $t = $cpu[$idx]
        foreach ($p in $t.env.PSObject.Properties) { $envs[$p.Name] = [string]$p.Value }
        $low = if ($idx -ne $found) { ", un cran plus bas pour $([Math]::Round($ramGb)) Go de mémoire vive" } else { '' }
        $reasons += "Processeur, $($hw.Threads) fils : $($t.nom)$low, $(Format-Env $t.env)."
    }
    # Mémoire de la carte.
    $card = @($m.carte.crans)
    if ($hw.VramBytes -gt 0 -and $card.Count -gt 0) {
        $t = $card[$card.Count - 1]
        foreach ($c in $card) { if ($null -eq $c.moins_de_go -or $vramGb -lt [double]$c.moins_de_go) { $t = $c; break } }
        foreach ($p in $t.env.PSObject.Properties) { $envs[$p.Name] = [string]$p.Value }
        $reasons += "Carte, $([Math]::Round($vramGb)) Go : $($t.nom), $(Format-Env $t.env)."
    } else { $reasons += 'Carte : mémoire inconnue, réglages pour 8 Go.' }
    # Mémoire vive : sous 24 Go les fenêtres de mémoire dans la carte ; sous 20 Go aussi des plafonds.
    if ($hw.RamBytes -gt 0) {
        $parts = @()
        if ($ramGb -lt [double]$Reglages.petite_ram.moins_de_go) {
            foreach ($p in $Reglages.petite_ram.env.PSObject.Properties) { $envs[$p.Name] = [string]$p.Value }
            $parts += "moins de $($Reglages.petite_ram.moins_de_go) Go, fenêtres de mémoire du jeu dans la carte ($(Format-Env $Reglages.petite_ram.env))"
        }
        if ($veryLow) {
            $capped = @()
            foreach ($p in $m.tres_petite_ram.plafonds.PSObject.Properties) {
                $cap = [double]$p.Value
                $now = if ($envs.Contains($p.Name)) { $envs[$p.Name] } elseif ($base.Contains($p.Name)) { $base[$p.Name] } else { $null }
                if ($null -eq $now -or [double]$now -gt $cap) { $envs[$p.Name] = [string]$cap; $capped += "$($p.Name -replace '^VICTORY_', '')=$cap" }
            }
            $parts += "moins de $($m.tres_petite_ram.moins_de_go) Go, fils un cran plus bas$(if ($capped) { ' et ' + ($capped -join ', ') })"
            $tips += $m.tres_petite_ram.conseil
        }
        $reasons += "Mémoire vive, $([Math]::Round($ramGb)) Go : $(if ($parts) { $parts -join ' ; ' } else { 'rien à réduire' })."
    }
    # Fabricant : DLSS, faits NVIDIA pour les shaders, avertissements.
    $vendor = Get-GpuVendor $hw.GpuName
    $v = $m.fabricants.$vendor
    if ($v.avertissement) { $warnings += $v.avertissement }
    # Seulement ce qui change vraiment par rapport aux groupes.
    $changed = [ordered]@{}
    foreach ($k in $envs.Keys) { if (-not ($base.Contains($k) -and $base[$k] -eq $envs[$k])) { $changed[$k] = $envs[$k] } }
    $label = @(if ($hw.GpuName) { ($hw.GpuName -replace '^(NVIDIA|AMD|Intel\(R\))\s+', '' -replace '^GeForce\s+', '').Trim() } else { 'carte inconnue' })
    if ($hw.VramBytes -gt 0) { $label += "$([Math]::Round($vramGb)) Go" }
    if ($hw.Threads -gt 0) { $label += "$($hw.Threads) fils" }
    if ($hw.RamBytes -gt 0) { $label += "$([Math]::Round($ramGb)) Go de RAM" }
    [pscustomobject]@{
        Label = $label -join ' · '; Vendor = $vendor; Env = $changed; Reasons = $reasons; Warnings = $warnings; Tips = $tips
        Dlss = [bool]$v.dlss; ShadersNvidia = [bool]$v.shaders_nvidia
    }
}

# Les variables que reçoit victory.exe : Set (dans l'ordre) et Remove. Sans PATH ni fichier : vérifiable à part.
function Get-GameEnvMap($s, $hw) {
    $set = [ordered]@{ VICTORY_CONFIG = $Toml }
    $seriesS = Get-SeriesSSettings
    foreach ($k in $seriesS.Keys) { $set[$k] = $seriesS[$k] }
    $prof = Get-HardwareProfile $hw
    foreach ($k in $prof.Env.Keys) { $set[$k] = $prof.Env[$k] }
    $set[$Reglages.pipelines_sautees.variable] = Get-SkipPipelines
    # DLSS (cartes RTX seulement) : voir « dlss » dans reglages.json.
    $d = $Reglages.dlss
    $remove = @($d.variable_dossier, $d.variable_mode)
    if ($prof.Dlss -and $s.dlss_mode -and $s.dlss_mode -ne 'off' -and $s.dlss_dir -and (Test-Path (Join-Path $s.dlss_dir $d.fichier_requis))) {
        $set[$d.variable_dossier] = $s.dlss_dir; $set[$d.variable_mode] = $s.dlss_mode
        foreach ($v in $d.retire) { $set.Remove($v) }
        $remove = @($d.retire)
    }
    [pscustomobject]@{ Set = $set; Remove = $remove }
}

# Prépare l'environnement de victory.exe (dans ce processus : les programmes lancés ensuite en héritent).
function Set-GameEnv($s) {
    $sdk = Get-ChildItem 'C:\Program Files (x86)\Windows Kits\10\bin\*\x64\dxil.dll' -ErrorAction SilentlyContinue | Sort-Object FullName | Select-Object -Last 1
    if ($sdk -and -not ($env:PATH -like "$($sdk.DirectoryName);*")) { $env:PATH = "$($sdk.DirectoryName);$env:PATH" }
    $map = Get-GameEnvMap $s (Get-Hardware)
    foreach ($k in $map.Set.Keys) { Set-Item "env:$k" $map.Set[$k] }
    foreach ($k in $map.Remove) { Remove-Item "env:$k" -ErrorAction SilentlyContinue }
}

# Les faits de la carte pour la traduction des shaders (NVIDIA) : voir « shaders » dans reglages.json.
function Get-ShaderHostEnv {
    $h = @{}
    foreach ($p in $Reglages.shaders.env_nvidia.PSObject.Properties) { $h[$p.Name] = [string]$p.Value }
    $h
}

# Threads pour « victory shaders » : tous les cœurs logiques sauf deux (au moins 2).
function Get-ShaderJobs { [Math]::Max([int]$Reglages.shaders.threads_min, [Environment]::ProcessorCount - [int]$Reglages.shaders.threads_laisses_libres) }

# ------------------------------------------------------------------ copie du jeu
# « jeu » dans reglages.json, le même contrôle que le lanceur graphique (GameRule::check dans settings.rs) :
# l'exe avec ses archives à côté. Renvoie $null si la copie est complète, sinon ce qui manque (en français).
function Test-GameCopy([string]$ExePath) {
    $j = $Reglages.jeu
    $dir = Split-Path -Parent $ExePath
    $rpfs = @(Get-ChildItem -LiteralPath $dir -Filter '*.rpf' -File -ErrorAction SilentlyContinue)
    $names = @($rpfs | ForEach-Object { $_.Name.ToLower() })
    $sizeGb = ($rpfs | Measure-Object Length -Sum).Sum / 1GB
    $missing = @()
    if (-not (Test-Path -LiteralPath $ExePath -PathType Leaf)) { $missing += $j.exe }
    $missing += @($j.archives_requises | Where-Object { $names -notcontains $_.ToLower() })
    $what = @()
    if ($missing) { $what += "il manque $($missing -join ', ')" }
    if ($rpfs.Count -lt [int]$j.archives_min) { $what += "$($rpfs.Count) archive$(if ($rpfs.Count -gt 1) { 's' }) .rpf sur $($j.archives_min) au moins" }
    if ($sizeGb -lt [double]$j.taille_min_go) { $what += ("{0:N0} Go d'archives sur {1} au moins (une copie complète fait ~130 Go)" -f $sizeGb, $j.taille_min_go) }
    if (-not $what) { return $null }
    "Copie du jeu incomplète dans $dir : $($what -join ' ; '). $($j.conseil)"
}

# Shaders mis dans le cache par une traduction : sa dernière ligne « N shaders in the cache, M do not translate ».
function Get-ShadersCount($Lines) {
    $n = $null
    foreach ($l in $Lines) { if ("$l" -match '^\s*(\d+) shaders in the cache, \d+ do not translate') { $n = [int]$Matches[1] } }
    $n
}
