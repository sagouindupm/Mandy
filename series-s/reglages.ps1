# Sagouindupm | VIctory: paths and lanceur.json of the console launcher Sagouindupm.ps1, and application of the
# Series S settings. The settings themselves are in reglages.json, the single source, which the graphical launcher
# (launcher/) reads too: a setting changed there applies to both.
# The measuring scripts (scripts/bench) call victory.exe directly and do not read these files.

$Root   = Split-Path -Parent $PSScriptRoot
# The downloaded version (scripts/package-release.ps1) has victory.exe in engine\; a repository, in target\release.
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
# Settings added after the first version of the launcher: present even in an old lanceur.json.
#   dlss_dir, dlss_mode : DLSS (Streamline folder, mode).
#   pipelines_parties   : number of sessions launched at the last successful "victory warm --used" (-1: never).
#   pipelines_liste     : "name:size" mark of the shared list built by that "warm" ('': none).
#   nv_cache_ok         : the player said they set the NVIDIA shader cache to "Unlimited".
#   ancien_plan         : power plan active before the graphical launcher changed it (to go back).
function Complete-Settings($s) {
    $defaults = [ordered]@{ dlss_dir = ''; dlss_mode = ''; pipelines_parties = -1; pipelines_liste = ''; nv_cache_ok = $false; ancien_plan = '' }
    foreach ($k in $defaults.Keys) {
        if (-not ($s.PSObject.Properties.Name -contains $k)) { $s | Add-Member -NotePropertyName $k -NotePropertyValue $defaults[$k] }
    }
    $s
}
function Save-Settings($s) { $s | ConvertTo-Json | Set-Content -Encoding utf8 $Conf }

# victory.toml: the game folder, read by victory.exe (VICTORY_CONFIG).
function Write-VictoryToml($s) {
    $dir = (Split-Path -Parent $s.game_exe) -replace "'", "''"
    "[paths]`ngame_dir = '$dir'`ntest_titles = []`n" | Set-Content -Encoding utf8 $Toml
    Save-Settings $s
}

# Series S settings, skipped pipelines, hardware profile, DLSS: series-s\reglages.json (also read by the graphical
# launcher). This file only applies it.
$Reglages = Get-Content (Join-Path $PSScriptRoot 'reglages.json') -Raw -Encoding UTF8 | ConvertFrom-Json

# The texts of reglages.json that this console shows, in English. reglages.json keeps the French ones (the graphical
# launcher shows them in its French window); the keys are the graphical launcher's (tier.<id>, vendorWarning.<id>...).
# A text missing here is shown as written in reglages.json.
$EnglishTexts = @{
    'tier.8_fils'                      = '8 threads or fewer'
    'tier.12_fils'                     = '6 cores / 12 threads (Maykzera''s settings)'
    'tier.16_fils'                     = '16 threads or more'
    'tier.6_go'                        = '6 GB or less'
    'tier.8_go'                        = '8 to 10 GB'
    'tier.12_go'                       = '11 GB and more'
    'vendorWarning.amd'                = 'AMD card: untested, generic Series S profile. Your measurements help us.'
    'vendorWarning.intel_arc'          = 'Intel Arc card: untested, generic Series S profile. Your measurements help us.'
    'vendorWarning.intel_integree'     = 'Integrated graphics: you need a dedicated graphics card with 4 GB or more.'
    'materiel.tres_petite_ram.conseil' = '16 GB of RAM: close your browser, Discord and other programs before playing.'
    'jeu.conseil'                      = 'Keep the whole game copy in its own folder (for example C:\Games\GTA6), not loose in Downloads.'
}
function Get-Text([string]$Key, [string]$Text) { if ($EnglishTexts.ContainsKey($Key)) { $EnglishTexts[$Key] } else { $Text } }

function Get-SeriesSSettings {
    $h = [ordered]@{}
    foreach ($g in $Reglages.groupes) { foreach ($p in $g.env.PSObject.Properties) { $h[$p.Name] = [string]$p.Value } }
    $h
}

# Skipped pipelines: the file of the ray tracing passes (relative to the repository root) then the extra hashes.
function Get-SkipPipelines {
    $rt = Join-Path $Root $Reglages.pipelines_sautees.fichier
    $skip = @($Reglages.pipelines_sautees.en_plus)
    if (Test-Path $rt) { $skip = @((Get-Content $rt -Raw).Trim()) + $skip }
    $skip -join ','
}

# The shared pipeline list ("pipelines_partagees" in reglages.json), $null if absent.
function Get-SharedPipelineList {
    $f = Join-Path $Root $Reglages.pipelines_partagees.fichier
    if (Test-Path $f) { $f } else { $null }
}

# Its "name:size" mark, to know which list was built (the same as the graphical launcher's).
function Get-SharedPipelineMark {
    $f = Get-SharedPipelineList
    if ($f) { $i = Get-Item $f; "$($i.Name):$($i.Length)" } else { '' }
}

# Total RAM in bytes (0 if unknown).
function Get-TotalRam {
    try { (Get-CimInstance Win32_ComputerSystem -ErrorAction Stop).TotalPhysicalMemory } catch { 0 }
}

# ------------------------------------------------------------------ hardware profile
# "materiel" in reglages.json, the same computation as launcher/src-tauri/src/profile.rs (scripts/check-profils.ps1
# compares the two).

# The graphics card: NVIDIA first, otherwise the one with the most memory (the driver's 64-bit size; AdapterRAM
# stops at 4 GB). The same choice as the graphical launcher (sys.rs).
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

# This PC (read once): RAM, card memory, CPU threads, card name. 0 = unknown.
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

# nvidia_rtx, nvidia, amd, intel_arc, intel_integree or inconnu, from the card's name.
function Get-GpuVendor([string]$Name) {
    $n = $Name.ToUpper()
    if ($n.Contains('NVIDIA') -or $n.Contains('GEFORCE')) { if ($n.Contains('RTX')) { return 'nvidia_rtx' } else { return 'nvidia' } }
    if ($n.Contains('AMD') -or $n.Contains('RADEON')) { return 'amd' }
    if ($n.Contains('INTEL')) {
        # Dedicated Arc cards have a number (A380, A770, B580); the Arc chips inside CPUs don't.
        $card = @($n -split '[^A-Z0-9]' | Where-Object { $_ -cmatch '^[AB][0-9]{3}$' }).Count -gt 0
        if ($n.Contains('ARC') -and $card) { return 'intel_arc' } else { return 'intel_integree' }
    }
    'inconnu'
}

function Format-Env($o) { ($o.PSObject.Properties | ForEach-Object { "$($_.Name -replace '^VICTORY_', '')=$($_.Value)" }) -join ', ' }

# This PC's profile: Env (what changes compared with the groups), Reasons, Warnings, Tips, Dlss, ShadersNvidia.
function Get-HardwareProfile($hw) {
    $m = $Reglages.materiel
    $base = Get-SeriesSSettings
    $envs = [ordered]@{}
    $reasons = @(); $warnings = @(); $tips = @()
    $ramGb = $hw.RamBytes / 1GB; $vramGb = $hw.VramBytes / 1GB
    $veryLow = $hw.RamBytes -gt 0 -and $ramGb -lt [double]$m.tres_petite_ram.moins_de_go

    # CPU: the first step under its limit; one step lower on a 16 GB PC.
    $cpu = @($m.processeur.crans)
    if ($hw.Threads -gt 0 -and $cpu.Count -gt 0) {
        $found = $cpu.Count - 1
        for ($i = 0; $i -lt $cpu.Count; $i++) { if ($null -eq $cpu[$i].moins_de_fils -or $hw.Threads -lt [int]$cpu[$i].moins_de_fils) { $found = $i; break } }
        $idx = if ($veryLow) { [Math]::Max(0, $found - [int]$m.tres_petite_ram.crans_de_fils_en_moins) } else { $found }
        $t = $cpu[$idx]
        foreach ($p in $t.env.PSObject.Properties) { $envs[$p.Name] = [string]$p.Value }
        $low = if ($idx -ne $found) { ", one step lower for $([Math]::Round($ramGb)) GB of RAM" } else { '' }
        $reasons += "CPU, $($hw.Threads) threads: $(Get-Text "tier.$($t.id)" $t.nom)$low, $(Format-Env $t.env)."
    }
    # Card memory.
    $card = @($m.carte.crans)
    if ($hw.VramBytes -gt 0 -and $card.Count -gt 0) {
        $t = $card[$card.Count - 1]
        foreach ($c in $card) { if ($null -eq $c.moins_de_go -or $vramGb -lt [double]$c.moins_de_go) { $t = $c; break } }
        foreach ($p in $t.env.PSObject.Properties) { $envs[$p.Name] = [string]$p.Value }
        $reasons += "Card, $([Math]::Round($vramGb)) GB: $(Get-Text "tier.$($t.id)" $t.nom), $(Format-Env $t.env)."
    } else { $reasons += 'Card: unknown memory, settings for 8 GB.' }
    # RAM: under 24 GB the memory windows on the card; under 20 GB caps too.
    if ($hw.RamBytes -gt 0) {
        $parts = @()
        if ($ramGb -lt [double]$Reglages.petite_ram.moins_de_go) {
            foreach ($p in $Reglages.petite_ram.env.PSObject.Properties) { $envs[$p.Name] = [string]$p.Value }
            $parts += "under $($Reglages.petite_ram.moins_de_go) GB, game memory windows on the card ($(Format-Env $Reglages.petite_ram.env))"
        }
        if ($veryLow) {
            $capped = @()
            foreach ($p in $m.tres_petite_ram.plafonds.PSObject.Properties) {
                $cap = [double]$p.Value
                $now = if ($envs.Contains($p.Name)) { $envs[$p.Name] } elseif ($base.Contains($p.Name)) { $base[$p.Name] } else { $null }
                if ($null -eq $now -or [double]$now -gt $cap) { $envs[$p.Name] = [string]$cap; $capped += "$($p.Name -replace '^VICTORY_', '')=$cap" }
            }
            $parts += "under $($m.tres_petite_ram.moins_de_go) GB, threads one step lower$(if ($capped) { ' and ' + ($capped -join ', ') })"
            $tips += Get-Text 'materiel.tres_petite_ram.conseil' $m.tres_petite_ram.conseil
        }
        $reasons += "RAM, $([Math]::Round($ramGb)) GB: $(if ($parts) { $parts -join '; ' } else { 'nothing to reduce' })."
    }
    # Vendor: DLSS, NVIDIA facts for the shaders, warnings.
    $vendor = Get-GpuVendor $hw.GpuName
    $v = $m.fabricants.$vendor
    if ($v.avertissement) { $warnings += Get-Text "vendorWarning.$vendor" $v.avertissement }
    # Only what really changes compared with the groups.
    $changed = [ordered]@{}
    foreach ($k in $envs.Keys) { if (-not ($base.Contains($k) -and $base[$k] -eq $envs[$k])) { $changed[$k] = $envs[$k] } }
    $label = @(if ($hw.GpuName) { ($hw.GpuName -replace '^(NVIDIA|AMD|Intel\(R\))\s+', '' -replace '^GeForce\s+', '').Trim() } else { 'unknown card' })
    if ($hw.VramBytes -gt 0) { $label += "$([Math]::Round($vramGb)) GB" }
    if ($hw.Threads -gt 0) { $label += "$($hw.Threads) threads" }
    if ($hw.RamBytes -gt 0) { $label += "$([Math]::Round($ramGb)) GB of RAM" }
    [pscustomobject]@{
        Label = $label -join ' · '; Vendor = $vendor; Env = $changed; Reasons = $reasons; Warnings = $warnings; Tips = $tips
        Dlss = [bool]$v.dlss; ShadersNvidia = [bool]$v.shaders_nvidia
    }
}

# The variables victory.exe receives: Set (in order) and Remove. No PATH or file: checkable on its own.
function Get-GameEnvMap($s, $hw) {
    $set = [ordered]@{ VICTORY_CONFIG = $Toml }
    $seriesS = Get-SeriesSSettings
    foreach ($k in $seriesS.Keys) { $set[$k] = $seriesS[$k] }
    $prof = Get-HardwareProfile $hw
    foreach ($k in $prof.Env.Keys) { $set[$k] = $prof.Env[$k] }
    $set[$Reglages.pipelines_sautees.variable] = Get-SkipPipelines
    # DLSS (RTX cards only): see "dlss" in reglages.json.
    $d = $Reglages.dlss
    $remove = @($d.variable_dossier, $d.variable_mode)
    if ($prof.Dlss -and $s.dlss_mode -and $s.dlss_mode -ne 'off' -and $s.dlss_dir -and (Test-Path (Join-Path $s.dlss_dir $d.fichier_requis))) {
        $set[$d.variable_dossier] = $s.dlss_dir; $set[$d.variable_mode] = $s.dlss_mode
        foreach ($v in $d.retire) { $set.Remove($v) }
        $remove = @($d.retire)
    }
    [pscustomobject]@{ Set = $set; Remove = $remove }
}

# Prepares victory.exe's environment (in this process: the programs started afterwards inherit it).
function Set-GameEnv($s) {
    $sdk = Get-ChildItem 'C:\Program Files (x86)\Windows Kits\10\bin\*\x64\dxil.dll' -ErrorAction SilentlyContinue | Sort-Object FullName | Select-Object -Last 1
    if ($sdk -and -not ($env:PATH -like "$($sdk.DirectoryName);*")) { $env:PATH = "$($sdk.DirectoryName);$env:PATH" }
    $map = Get-GameEnvMap $s (Get-Hardware)
    foreach ($k in $map.Set.Keys) { Set-Item "env:$k" $map.Set[$k] }
    foreach ($k in $map.Remove) { Remove-Item "env:$k" -ErrorAction SilentlyContinue }
}

# The card's facts for the shader translation (NVIDIA): see "shaders" in reglages.json.
function Get-ShaderHostEnv {
    $h = @{}
    foreach ($p in $Reglages.shaders.env_nvidia.PSObject.Properties) { $h[$p.Name] = [string]$p.Value }
    $h
}

# Threads for "victory shaders": all logical cores but two (at least 2).
function Get-ShaderJobs { [Math]::Max([int]$Reglages.shaders.threads_min, [Environment]::ProcessorCount - [int]$Reglages.shaders.threads_laisses_libres) }

# ------------------------------------------------------------------ game copy
# "jeu" in reglages.json, the same check as the graphical launcher (GameRule::check in settings.rs): the exe with
# its archives next to it. Returns $null if the copy is complete, otherwise what is missing.
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
    if ($missing) { $what += "missing $($missing -join ', ')" }
    if ($rpfs.Count -lt [int]$j.archives_min) { $what += "$($rpfs.Count) .rpf archive$(if ($rpfs.Count -ne 1) { 's' }) out of at least $($j.archives_min)" }
    if ($sizeGb -lt [double]$j.taille_min_go) { $what += ("{0:N0} GB of archives out of at least {1} (a complete copy is ~130 GB)" -f $sizeGb, $j.taille_min_go) }
    if (-not $what) { return $null }
    "Incomplete game copy in ${dir}: $($what -join '; '). $(Get-Text 'jeu.conseil' $j.conseil)"
}

# Shaders put in the cache by a translation: its last line "N shaders in the cache, M do not translate".
function Get-ShadersCount($Lines) {
    $n = $null
    foreach ($l in $Lines) { if ("$l" -match '^\s*(\d+) shaders in the cache, \d+ do not translate') { $n = [int]$Matches[1] } }
    $n
}
