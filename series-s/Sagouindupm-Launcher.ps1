# Sagouindupm | VIctory : lanceur graphique pour débutant (Windows PowerShell 5.1 + WinForms, rien à compiler).
# Démarré par Sagouindupm-Launcher.bat. Facultatif : le lanceur console Sagouindupm.bat reste pour les
# utilisateurs avancés, et les scripts de mesure appellent victory.exe directement.
#
# Trois onglets dans l'en-tête :
#   Jouer        : le jeu, le mode d'image (réglages Series S ou DLSS), le gros bouton JOUER.
#   Installation : un tableau de bord de 12 tuiles (vert prêt, orange à vérifier, rouge à faire, bleu info).
#                  Un clic sur une tuile la répare ; « Tout préparer pour moi » enchaîne ce qui manque.
#   Aide         : l'ordre des étapes, pourquoi on a moins d'images par seconde, où sont les fichiers.
#
# Les réglages (Series S, ray tracing, 24 Go, DLSS) viennent de reglages.ps1, comme pour le lanceur console.
# Les longues tâches (compilation, shaders, pipelines, partie) tournent dans des processus à part : leur sortie
# va dans un fichier de %LOCALAPPDATA%\Sagouindupm\journaux, que le lanceur relit toutes les 400 ms.
# Les vérifications lentes (WMI, nvidia-smi, recherche du jeu) tournent dans un runspace : la fenêtre ne gèle pas.

$ErrorActionPreference = 'Continue'
Add-Type -AssemblyName System.Windows.Forms, System.Drawing
[System.Windows.Forms.Application]::EnableVisualStyles()
. (Join-Path $PSScriptRoot 'reglages.ps1')
# La fenêtre console est cachée (-WindowStyle Hidden) : une erreur au démarrage est montrée et écrite au journal.
trap {
    $msg = "Le lanceur graphique s'est arrêté : $($_.Exception.Message)`n(ligne $($_.InvocationInfo.ScriptLineNumber))"
    try { Add-Content -Path (Join-Path $Logs 'lanceur_graphique.txt') -Value $msg -Encoding utf8 } catch { }
    [void][System.Windows.Forms.MessageBox]::Show($msg, 'Sagouindupm | VIctory', 'OK', 'Error')
    break
}

$script:S = Complete-Settings (Get-Settings)
$script:Facts = $null
$script:FactsTask = $null
$script:FactsCallbacks = @()
$script:SearchTask = $null
$script:Job = $null
$script:Queue = $null
$script:LastGameLog = $null
$UiLog = Join-Path $Logs 'lanceur_graphique.txt'
$Title_ = 'Sagouindupm | VIctory'

# ---------------------------------------------------------------- couleurs, polices
function New-Color($r, $g, $b) { [System.Drawing.Color]::FromArgb($r, $g, $b) }
$Pal = @{
    Head = New-Color 44 8 52; Accent = New-Color 139 0 139; AccentHover = New-Color 166 30 166; Orchid = New-Color 218 140 222
    Body = New-Color 245 242 247; Card = [System.Drawing.Color]::White; Text = New-Color 32 28 38; Muted = New-Color 96 92 104
    OkBg = New-Color 225 243 229; OkFg = New-Color 28 118 52
    WarnBg = New-Color 255 240 212; WarnFg = New-Color 170 98 0
    BadBg = New-Color 252 225 225; BadFg = New-Color 186 32 32
    InfoBg = New-Color 230 234 248; InfoFg = New-Color 52 66 140
    WaitBg = New-Color 236 234 238; WaitFg = New-Color 110 106 118
    LogBg = New-Color 26 20 30; LogFg = New-Color 220 214 226
}
function New-Font($size, [System.Drawing.FontStyle]$style = 'Regular', $family = 'Segoe UI') { New-Object System.Drawing.Font($family, [single]$size, $style) }

# ---------------------------------------------------------------- journal du lanceur
function Write-UiLog([string]$msg) {
    $line = '[{0:HH:mm:ss}] {1}' -f (Get-Date), $msg
    if ($script:txtLog) { Add-LogText $line }
    try { Add-Content -Path $UiLog -Value $line -Encoding utf8 } catch { }
}
function Add-LogText([string]$line) {
    $script:txtLog.AppendText($line + "`r`n")
    if ($script:txtLog.TextLength -gt 300000) {
        $script:txtLog.Text = $script:txtLog.Text.Substring($script:txtLog.TextLength - 200000)
        $script:txtLog.SelectionStart = $script:txtLog.TextLength; $script:txtLog.ScrollToCaret()
    }
}
function Show-Message([string]$text, [string]$buttons = 'OK', [string]$icon = 'Information') {
    [System.Windows.Forms.MessageBox]::Show($script:form, $text, $Title_, $buttons, $icon)
}
function Save-S { Save-Settings $script:S }

# ---------------------------------------------------------------- faits sur le PC (runspace, sans geler la fenêtre)
$FactsScript = {
    $f = @{}
    # Rust : cargo dans le PATH, ou là où rustup l'installe (pas encore dans le PATH juste après l'installation)
    $cargo = (Get-Command cargo.exe -ErrorAction SilentlyContinue | Select-Object -First 1).Source
    foreach ($cargoDir in @($env:CARGO_HOME, (Join-Path $env:USERPROFILE '.cargo'))) {
        if (-not $cargo -and $cargoDir -and (Test-Path (Join-Path $cargoDir 'bin\cargo.exe'))) { $cargo = Join-Path $cargoDir 'bin\cargo.exe' }
    }
    $f.Cargo = $cargo
    # Outils C++ de Visual Studio (vswhere, sinon link.exe de MSVC) et SDK Windows
    $vc = $null
    $vswhere = Join-Path ${env:ProgramFiles(x86)} 'Microsoft Visual Studio\Installer\vswhere.exe'
    if (Test-Path $vswhere) {
        $vc = & $vswhere -products * -latest -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -property installationPath 2>$null | Select-Object -First 1
    }
    if (-not $vc) {
        $l = Get-Command link.exe -ErrorAction SilentlyContinue | Where-Object { $_.Source -match 'MSVC' } | Select-Object -First 1
        if ($l) { $vc = $l.Source }
    }
    $f.VcTools = $vc
    $f.WinSdk = [bool](Get-ChildItem (Join-Path ${env:ProgramFiles(x86)} 'Windows Kits\10\Lib\*\um\x64\kernel32.lib') -ErrorAction SilentlyContinue | Select-Object -First 1)
    $f.Winget = (Get-Command winget.exe -ErrorAction SilentlyContinue | Select-Object -First 1).Source
    # Mémoire vive, mémoire virtuelle gérée par Windows
    try {
        $cs = Get-CimInstance Win32_ComputerSystem -ErrorAction Stop
        $f.Ram = [double]$cs.TotalPhysicalMemory; $f.AutoPagefile = [bool]$cs.AutomaticManagedPagefile
    } catch { $f.Ram = 0; $f.AutoPagefile = $null }
    # Carte graphique : la NVIDIA d'abord, sinon celle qui a le plus de mémoire
    $gpus = @(Get-CimInstance Win32_VideoController -ErrorAction SilentlyContinue | Where-Object { $_.Name -notmatch 'Basic Display|Remote|Virtual|Parsec|Meta Virtual' })
    $gpu = $gpus | Where-Object { $_.Name -match 'NVIDIA' } | Select-Object -First 1
    if (-not $gpu) { $gpu = $gpus | Sort-Object AdapterRAM -Descending | Select-Object -First 1 }
    $f.GpuName = if ($gpu) { [string]$gpu.Name } else { '' }
    $f.Nvidia = $f.GpuName -match 'NVIDIA'
    # Mémoire de la carte : AdapterRAM plafonne à 4 Go, le registre du pilote donne la vraie taille
    $vram = 0
    Get-ItemProperty 'HKLM:\SYSTEM\CurrentControlSet\Control\Class\{4d36e968-e325-11ce-bfc1-08002be10318}\0*' -ErrorAction SilentlyContinue | ForEach-Object {
        $q = $_.'HardwareInformation.qwMemorySize'
        if ($_.DriverDesc -eq $f.GpuName -and $q) { try { $vram = [double]$q } catch { } }
    }
    if (-not $vram -and $gpu) { $vram = [double]$gpu.AdapterRAM }
    $f.Vram = $vram
    # Resizable BAR : taille de BAR1 selon nvidia-smi (256 Mo = désactivé)
    $f.Bar1MB = 0
    $smi = (Get-Command nvidia-smi.exe -ErrorAction SilentlyContinue | Select-Object -First 1).Source
    if (-not $smi) { $c = Join-Path $env:SystemRoot 'System32\nvidia-smi.exe'; if (Test-Path $c) { $smi = $c } }
    if ($smi) {
        $inBar = $false
        foreach ($line in (& $smi -q -d MEMORY 2>$null)) {
            if ($line -match 'BAR1') { $inBar = $true; continue }
            if ($inBar -and $line -match 'Total\s*:\s*(\d+)\s*MiB') { $f.Bar1MB = [int]$Matches[1]; break }
        }
    }
    # Alimentation : mode actif (et, sur les portables récents, le mode « Performances optimales » superposé)
    $act = (& powercfg.exe /getactivescheme 2>$null) -join ' '
    $f.SchemeGuid = if ($act -match '([0-9a-fA-F]{8}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{12})') { $Matches[1].ToLower() } else { '' }
    $f.SchemeName = if ($act -match '\(([^)]+)\)\s*$') { $Matches[1] } else { '' }
    $overlay = (Get-ItemProperty 'HKLM:\SYSTEM\CurrentControlSet\Control\Power\User\PowerSchemes' -ErrorAction SilentlyContinue).ActiveOverlayAcPowerScheme
    $f.SchemeGood = ($f.SchemeGuid -in @('8c5e7fda-e8bf-4a96-9a85-a6e23a8c635c', 'e9a42b02-d5df-448d-aa00-03f14749eb61')) -or ($overlay -eq 'ded574b5-45a0-4f42-8737-46345c09c238')
    $f.Battery = [bool](Get-CimInstance Win32_Battery -ErrorAction SilentlyContinue | Select-Object -First 1)
    # Cache de shaders du pilote NVIDIA
    $f.DxCache = [double]0
    $dx = Join-Path $env:LOCALAPPDATA 'NVIDIA\DXCache'
    if (Test-Path $dx) {
        try { foreach ($fi in (New-Object System.IO.DirectoryInfo $dx).EnumerateFiles('*', 'AllDirectories')) { $f.DxCache += $fi.Length } } catch { }
    }
    # Pipelines construites un jour par « victory warm » (cache par défaut de victory.exe)
    $f.WarmDone = Test-Path (Join-Path $env:LOCALAPPDATA 'victory\cache\pipelines\warm-done.txt')
    $f
}

# Recherche de game_scarlett_master.exe : Téléchargements, Bureau, Documents, dossiers de jeux courants et
# racines des disques, sur 3 niveaux de dossiers au plus.
$SearchScript = {
    $name = 'game_scarlett_master.exe'
    $skip = @('Windows', '$Recycle.Bin', 'System Volume Information', 'WindowsApps', 'AppData', 'ProgramData',
        'Recovery', 'node_modules', '.git', 'Windows Kits', 'Microsoft Visual Studio', 'PerfLogs', 'Config.Msi', '.cargo', '.rustup')
    $starts = New-Object System.Collections.Generic.List[string]
    foreach ($p in @((Join-Path $env:USERPROFILE 'Downloads'), [Environment]::GetFolderPath('Desktop'), [Environment]::GetFolderPath('MyDocuments'))) {
        if ($p -and (Test-Path $p)) { $starts.Add($p) }
    }
    foreach ($d in [System.IO.DriveInfo]::GetDrives()) {
        if (-not $d.IsReady -or ($d.DriveType -ne 'Fixed' -and $d.DriveType -ne 'Removable')) { continue }
        foreach ($sub in 'Games', 'Jeux', 'XboxGames', 'SteamLibrary', 'Epic Games', 'Program Files', 'Program Files (x86)') {
            $p = Join-Path $d.RootDirectory.FullName $sub
            if (Test-Path $p) { $starts.Add($p) }
        }
        $starts.Add($d.RootDirectory.FullName)
    }
    $found = New-Object System.Collections.Generic.List[string]
    $seen = @{}
    foreach ($start in $starts) {
        $queue = New-Object System.Collections.Generic.Queue[string]
        $queue.Enqueue("0|$start")
        while ($queue.Count -gt 0) {
            $item = $queue.Dequeue(); $cut = $item.IndexOf('|')
            $depth = [int]$item.Substring(0, $cut); $dir = $item.Substring($cut + 1)
            if ($seen.ContainsKey($dir)) { continue }
            $seen[$dir] = $true
            $exe = Join-Path $dir $name
            if ([System.IO.File]::Exists($exe) -and -not $found.Contains($exe)) { $found.Add($exe) }
            if ($depth -ge 3) { continue }
            try {
                foreach ($sub in [System.IO.Directory]::EnumerateDirectories($dir)) {
                    if ($skip -contains [System.IO.Path]::GetFileName($sub)) { continue }
                    try { if ([System.IO.File]::GetAttributes($sub) -band [System.IO.FileAttributes]::ReparsePoint) { continue } } catch { continue }
                    $queue.Enqueue("$($depth + 1)|$sub")
                }
            } catch { }
        }
    }
    , $found.ToArray()
}

function Start-Runspace([scriptblock]$code) {
    $ps = [powershell]::Create()
    [void]$ps.AddScript($code.ToString())
    @{ PS = $ps; Handle = $ps.BeginInvoke() }
}
function Receive-Runspace($task) {
    try {
        $r = $task.PS.EndInvoke($task.Handle)
        # .psobject.BaseObject : sur une table, .BaseObject chercherait une clé de ce nom.
        if ($r.Count -gt 0) { return , $r[$r.Count - 1].psobject.BaseObject }
    } catch { Write-UiLog "Vérification impossible : $($_.Exception.Message)" }
    finally { $task.PS.Dispose() }
    $null
}
# Relance les vérifications ; $then est appelé quand elles sont finies.
function Request-Facts([scriptblock]$then) {
    if ($then) { $script:FactsCallbacks += $then }
    if (-not $script:FactsTask) { $script:FactsTask = Start-Runspace $FactsScript; Update-All }
}

# ---------------------------------------------------------------- tâches longues (processus à part)
# Commande lancée par cmd, sortie redirigée vers $Log : PowerShell 5.1 ne transforme pas les lignes en erreurs,
# et le lanceur relit le fichier pendant que la tâche tourne.
function Start-CmdJob {
    param([string]$Title, [string]$Command, [string]$Log, [string]$WorkDir = $PSScriptRoot, [hashtable]$ExtraEnv = @{},
        [scriptblock]$OnExit, [switch]$Game)
    $psi = New-Object System.Diagnostics.ProcessStartInfo $env:ComSpec
    $psi.Arguments = '/d /s /c "' + $Command + ' > "' + $Log + '" 2>&1"'
    $psi.UseShellExecute = $false; $psi.CreateNoWindow = $true; $psi.WorkingDirectory = $WorkDir
    foreach ($k in $ExtraEnv.Keys) { $psi.EnvironmentVariables[$k] = $ExtraEnv[$k] }
    Write-UiLog "$Title : $Command"
    Write-UiLog "  (journal : $Log)"
    try { $p = [System.Diagnostics.Process]::Start($psi) } catch { Write-UiLog "Impossible de lancer : $($_.Exception.Message)"; return $false }
    $script:Job = @{ Title = $Title; Proc = $p; Log = $Log; Offset = [long]0; Decoder = [System.Text.Encoding]::UTF8.GetDecoder()
        Partial = ''; OnExit = $OnExit; Game = $Game.IsPresent; Started = Get-Date; Last = ''; Cancelled = $false }
    Set-Busy
    $true
}
# Commande dans une fenêtre visible (winget : Windows et l'installeur posent leurs questions au joueur).
function Start-WindowJob([string]$Title, [string]$Command, [scriptblock]$OnExit) {
    $psi = New-Object System.Diagnostics.ProcessStartInfo $env:ComSpec
    $psi.Arguments = '/d /s /c "' + $Command + ' & echo. & pause"'
    $psi.UseShellExecute = $true
    Write-UiLog "$Title : $Command"
    Write-UiLog '  Une fenêtre noire s''est ouverte : suivez-la (Windows peut demander une autorisation), puis appuyez sur une touche pour la fermer.'
    try { $p = [System.Diagnostics.Process]::Start($psi) } catch { Write-UiLog "Impossible de lancer : $($_.Exception.Message)"; return $false }
    $script:Job = @{ Title = $Title; Proc = $p; Log = $null; OnExit = $OnExit; Game = $false; Started = Get-Date; Last = 'fenêtre d''installation ouverte'; Cancelled = $false }
    Set-Busy
    $true
}
function Read-JobLog($j) {
    if (-not $j.Log -or -not (Test-Path $j.Log)) { return }
    $fs = $null
    try {
        $fs = [System.IO.File]::Open($j.Log, 'Open', 'Read', 'ReadWrite, Delete')
        # Une partie écrit beaucoup : on ne lit que la fin.
        if ($j.Game -and ($fs.Length - $j.Offset) -gt 512KB) { $j.Offset = $fs.Length - 64KB; $j.Partial = ''; $j.Decoder = [System.Text.Encoding]::UTF8.GetDecoder() }
        if ($fs.Length -le $j.Offset) { return }
        [void]$fs.Seek($j.Offset, 'Begin')
        $n = [int][Math]::Min($fs.Length - $j.Offset, 1MB)
        $buf = New-Object byte[] $n
        $read = $fs.Read($buf, 0, $n)
        $j.Offset += $read
        $chars = New-Object char[] ($j.Decoder.GetCharCount($buf, 0, $read))
        [void]$j.Decoder.GetChars($buf, 0, $read, $chars, 0)
        $text = $j.Partial + (New-Object string (, $chars))
    } catch { return } finally { if ($fs) { $fs.Dispose() } }
    $lines = $text -split "`r?`n"
    $j.Partial = $lines[-1]
    foreach ($line in $lines[0..($lines.Count - 2)]) { Show-JobLine $j $line }
}
function Show-JobLine($j, [string]$line) {
    if (-not $line.Trim()) { return }
    if ($j.Game) {
        # « 9.8 fps, 102.0 ms/frame (...) » une fois par seconde : on n'affiche que le chiffre.
        if ($line -match '([\d.]+) fps, ([\d.]+) ms/frame') {
            $script:lblPlayState.Text = "En jeu : $($Matches[1]) images/s ($($Matches[2]) ms par image)"
        }
        return
    }
    $j.Last = $line.Trim()
    Add-LogText $line
}
function Stop-Job {
    $j = $script:Job
    if (-not $j) { return }
    $what = if ($j.Game) { 'Fermer le jeu de force ?' } else { "Arrêter « $($j.Title) » ?" }
    if ((Show-Message $what 'YesNo' 'Question') -ne 'Yes') { return }
    $j.Cancelled = $true
    & taskkill.exe /PID $j.Proc.Id /T /F 2>&1 | Out-Null
    Write-UiLog "« $($j.Title) » arrêté."
}
function Update-Job {
    $j = $script:Job
    if (-not $j) { return }
    Read-JobLog $j
    $el = (Get-Date) - $j.Started
    $script:lblTask.Text = '{0} : en cours depuis {1:00}:{2:00}   {3}' -f $j.Title, [Math]::Floor($el.TotalMinutes), $el.Seconds, $j.Last
    if (-not $j.Proc.HasExited) { return }
    Read-JobLog $j
    if ($j.Partial) { Show-JobLine $j $j.Partial; $j.Partial = '' }
    $code = $j.Proc.ExitCode
    $script:Job = $null
    $script:lblTask.Text = "$($j.Title) : terminé (code $code)."
    Write-UiLog "$($j.Title) : terminé (code $code)."
    Set-Busy
    if ($j.Cancelled -and $script:Queue) { Stop-Queue 'Tâche arrêtée.' }
    if ($j.OnExit) { & $j.OnExit $code $j }
    Update-All
}

# ---------------------------------------------------------------- les 12 vérifications
$Checks = @(
    @{ Id = 'rust'; N = 1; Title = 'Rust (cargo)'; Desc = 'Le langage de VIctory : nécessaire pour le compiler.' }
    @{ Id = 'vs'; N = 2; Title = 'Outils C++ Microsoft'; Desc = 'Visual Studio Build Tools (C++) et SDK Windows : l''éditeur de liens de Rust.' }
    @{ Id = 'build'; N = 3; Title = 'VIctory compilé'; Desc = 'target\release\victory.exe, le programme qui fait tourner le jeu.' }
    @{ Id = 'game'; N = 4; Title = 'Dossier du jeu'; Desc = 'game_scarlett_master.exe de VOTRE copie du jeu.' }
    @{ Id = 'shaders'; N = 5; Title = 'Shaders traduits'; Desc = 'Une fois (15-30 min) : évite écrans noirs et à-coups en jeu.' }
    @{ Id = 'pipes'; N = 6; Title = 'Pipelines construites'; Desc = 'Après une partie : ~5 fps deviennent ~10 fps sur la machine de référence.' }
    @{ Id = 'nvcache'; N = 7; Title = 'Cache de shaders NVIDIA'; Desc = 'Limité à 4 Go : 1re cause des lancements lents qui se répètent.' }
    @{ Id = 'power'; N = 8; Title = 'Alimentation'; Desc = 'Sur secteur et en performances élevées (portables surtout).' }
    @{ Id = 'pagefile'; N = 9; Title = 'Mémoire virtuelle'; Desc = 'Gérée par Windows : une petite taille fixe fait planter ou ralentir.' }
    @{ Id = 'rebar'; N = 10; Title = 'Resizable BAR'; Desc = 'Le processeur écrit dans toute la mémoire de la carte (BIOS).' }
    @{ Id = 'ram'; N = 11; Title = 'Mémoire vive'; Desc = 'Moins de 24 Go : le lanceur s''adapte, fermez les autres programmes.' }
    @{ Id = 'gpu'; N = 12; Title = 'Carte graphique'; Desc = 'DLSS possible sur les cartes RTX.' }
)
$CheckById = @{}; foreach ($chk in $Checks) { $CheckById[$chk.Id] = $chk }

function Format-GB([double]$bytes) { '{0:N1} Go' -f ($bytes / 1GB) }
function Test-Game { [bool]($script:S.game_exe -and (Test-Path $script:S.game_exe)) }
function Test-DlssDir { [bool]($script:S.dlss_dir -and (Test-Path (Join-Path $script:S.dlss_dir 'sl.interposer.dll'))) }
function Get-PowerLine { [string][System.Windows.Forms.SystemInformation]::PowerStatus.PowerLineStatus }
function Get-SchemeLabel([string]$guid, [string]$fallback) {
    switch ($guid) {
        '381b4222-f694-41f0-9685-ff5bb260df2e' { 'Utilisation normale' }
        'a1841308-3541-4fab-bc81-f71556f20b4a' { 'Économie d''énergie' }
        '8c5e7fda-e8bf-4a96-9a85-a6e23a8c635c' { 'Performances élevées' }
        'e9a42b02-d5df-448d-aa00-03f14749eb61' { 'Performances optimales' }
        default { $fallback }
    }
}

# État d'une vérification : Level ok / warn / bad / info / wait, Text, Action (texte du lien), Auto (fait par
# « Tout préparer pour moi »).
function Get-CheckState([string]$id) {
    $f = $script:Facts; $s = $script:S
    $st = @{ Level = 'wait'; Text = 'Vérification…'; Action = ''; Auto = $false }
    $exeThere = Test-Path $Exe
    switch ($id) {
        'rust' {
            if (-not $f) { break }
            if ($f.Cargo) { $st = @{ Level = 'ok'; Text = 'Installé'; Action = ''; Auto = $false } }
            elseif ($exeThere) { $st = @{ Level = 'info'; Text = 'Pas nécessaire : VIctory est déjà compilé'; Action = 'Installer quand même'; Auto = $false } }
            else { $st = @{ Level = 'bad'; Text = 'Absent'; Action = 'Installer Rust'; Auto = $true } }
        }
        'vs' {
            if (-not $f) { break }
            if ($f.VcTools -and $f.WinSdk) { $st = @{ Level = 'ok'; Text = 'C++ et SDK Windows présents'; Action = ''; Auto = $false } }
            elseif ($exeThere) { $st = @{ Level = 'info'; Text = 'Pas nécessaire : VIctory est déjà compilé'; Action = 'Installer quand même'; Auto = $false } }
            elseif ($f.VcTools) { $st = @{ Level = 'bad'; Text = 'SDK Windows absent'; Action = 'Installer'; Auto = $true } }
            else { $st = @{ Level = 'bad'; Text = 'Absents'; Action = 'Installer (gros téléchargement)'; Auto = $true } }
        }
        'build' {
            if ($exeThere) { $st = @{ Level = 'ok'; Text = ('Compilé le {0:dd/MM/yyyy HH:mm}' -f (Get-Item $Exe).LastWriteTime); Action = 'Recompiler'; Auto = $false } }
            else { $st = @{ Level = 'bad'; Text = 'Pas encore compilé'; Action = 'Compiler (10-20 min)'; Auto = $true } }
        }
        'game' {
            if (Test-Game) { $st = @{ Level = 'ok'; Text = (Split-Path -Leaf (Split-Path -Parent $s.game_exe)); Action = 'Changer de dossier'; Auto = $false } }
            else { $st = @{ Level = 'bad'; Text = 'Jeu pas encore trouvé'; Action = 'Chercher le jeu'; Auto = $true } }
        }
        'shaders' {
            if ($s.shaders_ready) { $st = @{ Level = 'ok'; Text = 'Traduits'; Action = 'Retraduire'; Auto = $false } }
            elseif (-not (Test-Game)) { $st = @{ Level = 'bad'; Text = 'Il faut d''abord le jeu (tuile 4)'; Action = 'Traduire'; Auto = $true } }
            else { $st = @{ Level = 'bad'; Text = 'Pas traduits'; Action = 'Traduire (15-30 min)'; Auto = $true } }
        }
        'pipes' {
            $at = [int]$s.pipelines_parties
            if ([int]$s.parties -le 0) { $st = @{ Level = 'warn'; Text = 'Jouez d''abord une partie'; Action = 'Pourquoi ?'; Auto = $false } }
            elseif ($at -lt 0) { $st = @{ Level = 'bad'; Text = "Pas construites ($($s.parties) partie(s) jouée(s))"; Action = 'Construire (quelques min)'; Auto = $true } }
            elseif ($at -lt [int]$s.parties) { $st = @{ Level = 'warn'; Text = 'Nouvelles pipelines notées depuis'; Action = 'Construire (quelques min)'; Auto = $true } }
            else { $st = @{ Level = 'ok'; Text = 'À jour'; Action = 'Reconstruire'; Auto = $false } }
        }
        'nvcache' {
            if (-not $f) { break }
            $size = Format-GB $f.DxCache
            if (-not $f.Nvidia) { $st = @{ Level = 'info'; Text = 'Non concerné (carte non NVIDIA)'; Action = ''; Auto = $false } }
            elseif ($s.nv_cache_ok) { $st = @{ Level = 'ok'; Text = "Illimitée (vous l'avez confirmé), $size"; Action = 'Revoir'; Auto = $false } }
            # Au-delà de 4,5 Go la limite de 4 Go du pilote n'est plus appliquée ; entre 3,5 et 4,5 Go le cache est sans doute plein.
            elseif ($f.DxCache -gt 4.5GB) { $st = @{ Level = 'ok'; Text = "$size : limite levée"; Action = 'Vérifier'; Auto = $false } }
            elseif ($f.DxCache -ge 3.5GB) { $st = @{ Level = 'bad'; Text = "$size : sans doute plein (4 Go)"; Action = 'Régler sur Illimitée'; Auto = $false } }
            else { $st = @{ Level = 'warn'; Text = "$size : à régler sur Illimitée"; Action = 'Régler sur Illimitée'; Auto = $false } }
        }
        'power' {
            if (-not $f) { break }
            $name = Get-SchemeLabel $f.SchemeGuid $f.SchemeName
            if ((Get-PowerLine) -eq 'Offline') { $st = @{ Level = 'bad'; Text = 'Sur batterie : branchez le chargeur'; Action = 'Pourquoi ?'; Auto = $false } }
            elseif ($f.SchemeGood) {
                $act = if ($s.ancien_plan) { 'Revenir à l''ancien mode' } else { '' }
                $st = @{ Level = 'ok'; Text = "Performances élevées$(if ($f.Battery) { ', sur secteur' })"; Action = $act; Auto = $false }
            }
            else { $st = @{ Level = 'warn'; Text = "Mode « $name »"; Action = 'Passer en performances élevées'; Auto = $true } }
        }
        'pagefile' {
            if (-not $f) { break }
            if ($null -eq $f.AutoPagefile) { $st = @{ Level = 'info'; Text = 'Inconnue'; Action = 'Voir comment faire'; Auto = $false } }
            elseif ($f.AutoPagefile) { $st = @{ Level = 'ok'; Text = 'Gérée par Windows'; Action = ''; Auto = $false } }
            else { $st = @{ Level = 'warn'; Text = 'Taille fixe'; Action = 'Voir comment faire (admin)'; Auto = $false } }
        }
        'rebar' {
            if (-not $f) { break }
            if ($f.Bar1MB -ge 1024) { $st = @{ Level = 'ok'; Text = "Actif (BAR1 $($f.Bar1MB) Mo)"; Action = 'En savoir plus'; Auto = $false } }
            elseif ($f.Bar1MB -gt 0) { $st = @{ Level = 'warn'; Text = "Désactivé (BAR1 $($f.Bar1MB) Mo) : voir le BIOS"; Action = 'En savoir plus'; Auto = $false } }
            else { $st = @{ Level = 'info'; Text = 'Inconnu : voir le BIOS'; Action = 'En savoir plus'; Auto = $false } }
        }
        'ram' {
            if (-not $f) { break }
            if ($f.Ram -le 0) { $st = @{ Level = 'info'; Text = 'Inconnue'; Action = ''; Auto = $false } }
            elseif ($f.Ram -ge 24GB) { $st = @{ Level = 'ok'; Text = (Format-GB $f.Ram); Action = ''; Auto = $false } }
            else { $st = @{ Level = 'warn'; Text = "$(Format-GB $f.Ram) : le lanceur s'adapte"; Action = 'Conseils'; Auto = $false } }
        }
        'gpu' {
            if (-not $f) { break }
            $txt = if ($f.GpuName) { $f.GpuName } else { 'Inconnue' }
            if ($f.Vram -gt 0) { $txt += ', ' + ('{0:N0} Go' -f ($f.Vram / 1GB)) }
            $act = if ($f.GpuName -match 'RTX') { 'Régler le DLSS' } else { 'Détails' }
            $st = @{ Level = 'info'; Text = $txt; Action = $act; Auto = $false }
        }
    }
    $st
}

# ---------------------------------------------------------------- réparer une vérification
# Résultat : 'started' (une tâche tourne, elle reprend la file à la fin), 'done', 'failed', 'skipped'.
function Invoke-Fix([string]$id, [switch]$Auto) {
    $f = $script:Facts; $s = $script:S
    $needsJob = @('rust', 'vs', 'build', 'shaders', 'pipes', 'game')
    if ($script:Job -and $needsJob -contains $id) {
        if (-not $Auto) { Show-Message "Une tâche est déjà en cours ($($script:Job.Title)). Attendez qu'elle finisse." | Out-Null }
        return 'failed'
    }
    switch ($id) {
        'rust' {
            if (-not $Auto -and $f -and $f.Cargo) { Show-Message "Rust est déjà installé :`n$($f.Cargo)" | Out-Null; return 'done' }
            if (-not ($f -and $f.Winget)) {
                Start-Process 'https://rustup.rs'
                Show-Message "winget est absent de ce Windows. La page de Rust s'est ouverte : téléchargez rustup-init.exe, lancez-le, puis cliquez « Revérifier »." | Out-Null
                return 'failed'
            }
            $ok = Start-WindowJob 'Installation de Rust' "`"$($f.Winget)`" install --id Rustlang.Rustup -e" { param($code, $j) Resume-Queue $true }
            if ($ok) { return 'started' } else { return 'failed' }
        }
        'vs' {
            if (-not ($f -and $f.Winget)) {
                Start-Process 'https://visualstudio.microsoft.com/visual-cpp-build-tools/'
                Show-Message "winget est absent de ce Windows. La page des Build Tools s'est ouverte : installez « Développement Desktop en C++ », puis cliquez « Revérifier »." | Out-Null
                return 'failed'
            }
            if (-not $Auto -and (Show-Message "Installer les outils C++ de Microsoft (Visual Studio Build Tools, plusieurs Go) ?`nWindows demandera une autorisation." 'YesNo' 'Question') -ne 'Yes') { return 'skipped' }
            $cmd = "`"$($f.Winget)`" install --id Microsoft.VisualStudio.2022.BuildTools -e --override `"--quiet --wait --add Microsoft.VisualStudio.Workload.VCTools --includeRecommended`""
            $ok = Start-WindowJob 'Installation des outils C++' $cmd { param($code, $j) Resume-Queue $true }
            if ($ok) { return 'started' } else { return 'failed' }
        }
        'build' {
            if (-not ($f -and $f.Cargo)) { Show-Message 'Installez d''abord Rust (tuile 1).' 'OK' 'Warning' | Out-Null; return 'failed' }
            if (-not $f.VcTools) { Show-Message 'Installez d''abord les outils C++ de Microsoft (tuile 2).' 'OK' 'Warning' | Out-Null; return 'failed' }
            if (-not $Auto -and (Show-Message "Compiler VIctory ? (10 à 20 minutes la première fois, le PC sera occupé)" 'YesNo' 'Question') -ne 'Yes') { return 'skipped' }
            $dir = Split-Path -Parent $f.Cargo
            if (-not ($env:PATH -like "*$dir*")) { $env:PATH = "$dir;$env:PATH" }
            $ok = Start-CmdJob -Title 'Compilation de VIctory' -Command "`"$($f.Cargo)`" build --release --locked -p victory" -Log (Join-Path $Logs 'compilation.txt') -WorkDir $Root -OnExit {
                param($code, $j)
                if ($code -eq 0 -and (Test-Path $Exe)) { Write-UiLog 'VIctory est compilé.' } else { Write-UiLog "La compilation a échoué : voir $Logs\compilation.txt" }
                Resume-Queue ($code -eq 0 -and (Test-Path $Exe))
            }
            if ($ok) { return 'started' } else { return 'failed' }
        }
        'game' {
            if ((Test-Game) -and -not $Auto) { if (Select-GameFile) { return 'done' } else { return 'skipped' } }
            Start-GameSearch
            return 'started'
        }
        'shaders' {
            if (-not (Test-Path $Exe)) { Show-Message 'Compilez d''abord VIctory (tuile 3).' 'OK' 'Warning' | Out-Null; return 'failed' }
            if (-not (Test-Game)) { Show-Message 'Indiquez d''abord où est le jeu (tuile 4).' 'OK' 'Warning' | Out-Null; return 'failed' }
            if (-not $Auto -and (Show-Message "Traduire tous les shaders du jeu ?`n15 à 30 minutes, une seule fois ; le PC sera très occupé." 'YesNo' 'Question') -ne 'Yes') { return 'skipped' }
            Set-GameEnv $s
            $src = Get-ShadersSource
            $extra = @{}
            if ($f -and $f.Nvidia) { $extra = Get-ShaderHostEnv }
            $ok = Start-CmdJob -Title 'Traduction des shaders' -Command "`"$Exe`" shaders `"$src`" --jobs $(Get-ShaderJobs)" -Log (Join-Path $Logs 'shaders_traduction.txt') -ExtraEnv $extra -OnExit {
                param($code, $j)
                if ($code -eq 0) { $script:S.shaders_ready = $true; Save-S; Write-UiLog 'Shaders prêts !' }
                else { Write-UiLog "La traduction s'est arrêtée (code $code), voir $Logs\shaders_traduction.txt" }
                Resume-Queue ($code -eq 0)
            }
            if ($ok) { return 'started' } else { return 'failed' }
        }
        'pipes' {
            if ([int]$s.parties -le 0) {
                if (-not $Auto) {
                    Show-Message ("Pendant une partie, le jeu note les pipelines qu'il crée (les programmes de dessin de la carte graphique).`n`n" +
                        "« Construire les pipelines » les prépare ensuite hors jeu : la partie suivante n'a plus d'à-coups ni de dessins manquants.`n`n" +
                        "Jouez donc une première partie (onglet Jouer), puis revenez ici.") | Out-Null
                }
                return 'skipped'
            }
            if (-not (Test-Path $Exe)) { Show-Message 'Compilez d''abord VIctory (tuile 3).' 'OK' 'Warning' | Out-Null; return 'failed' }
            Set-GameEnv $s
            $ok = Start-CmdJob -Title 'Construction des pipelines' -Command "`"$Exe`" warm --used" -Log (Join-Path $Logs 'pipelines_warm.txt') -OnExit {
                param($code, $j)
                if ($code -eq 0) { $script:S.pipelines_parties = [int]$script:S.parties; Save-S; Write-UiLog 'Pipelines prêtes : la prochaine partie démarre sans à-coups.' }
                else { Write-UiLog "La construction s'est arrêtée (code $code), voir $Logs\pipelines_warm.txt" }
                Resume-Queue ($code -eq 0)
            }
            if ($ok) { return 'started' } else { return 'failed' }
        }
        'nvcache' { Show-NvidiaCacheGuide; return 'done' }
        'power' { return (Set-PowerMode -Auto:$Auto) }
        'pagefile' {
            $r = Show-Message ("Windows doit gérer seul la mémoire virtuelle (fichier d'échange). Une petite taille fixe fait planter ou ralentir le jeu.`n`n" +
                "Cliquez OK : la fenêtre « Options de performances » s'ouvre (Windows demande une autorisation d'administrateur). Puis :`n" +
                "1. onglet « Avancé », Mémoire virtuelle, « Modifier… » ;`n" +
                "2. cochez « Gérer automatiquement la taille du fichier d'échange pour tous les lecteurs » ;`n" +
                "3. OK, puis redémarrez le PC.`n`nLe lanceur ne change pas ce réglage lui-même.") 'OKCancel'
            if ($r -eq 'OK') { try { Start-Process (Join-Path $env:SystemRoot 'System32\SystemPropertiesPerformance.exe') } catch { Write-UiLog "Ouverture refusée : $($_.Exception.Message)" } }
            return 'done'
        }
        'rebar' {
            $state = (Get-CheckState 'rebar').Text
            Show-Message ("Resizable BAR : $state.`n`n" +
                "Il laisse le processeur écrire directement dans toute la mémoire de la carte graphique. Sur les PC à moins de 24 Go de RAM, " +
                "le lanceur y place une partie de la mémoire du jeu (VICTORY_WINDOWS_BAR_MB=3072).`n`n" +
                "Pour l'activer : dans le BIOS de la carte mère, « Above 4G Decoding » et « Re-Size BAR Support » sur Enabled " +
                "(souvent avec le CSM désactivé). Voir le manuel de votre carte mère. Le lanceur ne touche pas au BIOS.") | Out-Null
            return 'done'
        }
        'ram' {
            $ram = if ($f) { Format-GB $f.Ram } else { '?' }
            Show-Message ("Mémoire vive : $ram.`n`n" +
                "Moins de 24 Go : le lanceur s'adapte tout seul (une partie de la mémoire du jeu va dans la carte graphique, " +
                "VICTORY_WINDOWS_BAR_MB=3072, ce qui évite le plantage 0x8007000E).`n`n" +
                "Avant de jouer, fermez le navigateur, Discord et les autres programmes.") | Out-Null
            return 'done'
        }
        'gpu' { Show-GpuInfo; return 'done' }
    }
    'skipped'
}

function Get-ShadersSource {
    $s = $script:S
    if ($s.shaders_dir -and (Test-Path $s.shaders_dir)) { return $s.shaders_dir }
    # Sans dossier extrait : le shaders.rpf du jeu, sinon tout le dossier du jeu (victory shaders y cherche les
    # conteneurs de shaders et les enregistrements SGD5 dans chaque fichier).
    $dir = Split-Path -Parent $s.game_exe
    $rpf = Join-Path $dir 'shaders.rpf'
    if (Test-Path $rpf) { return $rpf }
    $dir
}

function Set-GamePath([string]$path) {
    $script:S.game_exe = $path
    Write-VictoryToml $script:S
    Write-UiLog "Jeu : $path"
    Update-All
}
function Select-GameFile {
    $dlg = New-Object System.Windows.Forms.OpenFileDialog
    $dlg.Title = 'Où est game_scarlett_master.exe (votre copie du jeu) ?'
    $dlg.Filter = 'Jeu (game_scarlett_master.exe)|game_scarlett_master.exe|Programmes (*.exe)|*.exe'
    if (Test-Game) { $dlg.InitialDirectory = Split-Path -Parent $script:S.game_exe }
    if ($dlg.ShowDialog($script:form) -eq 'OK') { Set-GamePath $dlg.FileName; return $true }
    $false
}
function Select-FromList([string]$prompt, [string[]]$items) {
    $d = New-Object System.Windows.Forms.Form
    $d.Text = $Title_; $d.Size = New-Object System.Drawing.Size(640, 320); $d.StartPosition = 'CenterParent'
    $d.FormBorderStyle = 'FixedDialog'; $d.MaximizeBox = $false; $d.MinimizeBox = $false; $d.Font = New-Font 9.5
    $l = New-Object System.Windows.Forms.Label; $l.Text = $prompt; $l.SetBounds(12, 10, 600, 24)
    $lb = New-Object System.Windows.Forms.ListBox; $lb.SetBounds(12, 38, 600, 190); $lb.HorizontalScrollbar = $true
    foreach ($i in $items) { [void]$lb.Items.Add($i) }
    $lb.SelectedIndex = 0
    $ok = New-Object System.Windows.Forms.Button; $ok.Text = 'Choisir'; $ok.SetBounds(412, 238, 96, 30); $ok.DialogResult = 'OK'
    $no = New-Object System.Windows.Forms.Button; $no.Text = 'Annuler'; $no.SetBounds(516, 238, 96, 30); $no.DialogResult = 'Cancel'
    $d.Controls.AddRange(@($l, $lb, $ok, $no)); $d.AcceptButton = $ok; $d.CancelButton = $no
    $r = $d.ShowDialog($script:form); $sel = $lb.SelectedItem; $d.Dispose()
    if ($r -eq 'OK') { return [string]$sel }
    $null
}
function Start-GameSearch {
    if ($script:SearchTask) { return }
    Write-UiLog 'Recherche de game_scarlett_master.exe (Téléchargements, Bureau, Documents, dossiers de jeux, racines des disques)…'
    $script:lblTask.Text = 'Recherche du jeu…'
    $script:SearchTask = Start-Runspace $SearchScript
    Set-Busy
}
function Complete-GameSearch([string[]]$found) {
    $found = @($found | Where-Object { $_ })
    $chosen = $null
    if ($found.Count -eq 1) { $chosen = $found[0]; Write-UiLog "Jeu trouvé : $chosen" }
    elseif ($found.Count -gt 1) { $chosen = Select-FromList "$($found.Count) copies du jeu trouvées : laquelle utiliser ?" $found }
    else {
        Write-UiLog 'Jeu introuvable automatiquement : choisissez le fichier.'
        Show-Message "Le jeu n'a pas été trouvé automatiquement.`nIndiquez le fichier game_scarlett_master.exe de VOTRE copie du jeu." | Out-Null
    }
    $script:lblTask.Text = ''
    if ($chosen) { Set-GamePath $chosen } else { [void](Select-GameFile) }
    Resume-Queue (Test-Game)
}

function Open-NvidiaPanel {
    $exe = Join-Path $env:ProgramFiles 'NVIDIA Corporation\Control Panel Client\nvcplui.exe'
    if (Test-Path $exe) { Start-Process $exe; return $true }
    $pkg = Get-AppxPackage -Name 'NVIDIACorp.NVIDIAControlPanel' -ErrorAction SilentlyContinue | Select-Object -First 1
    if ($pkg) { Start-Process explorer.exe "shell:AppsFolder\$($pkg.PackageFamilyName)!NVIDIACorp.NVIDIAControlPanel"; return $true }
    $false
}
function Show-NvidiaCacheGuide {
    $f = $script:Facts
    if ($f -and -not $f.Nvidia) { Show-Message 'Carte non NVIDIA : rien à régler ici.' | Out-Null; return }
    $size = if ($f) { Format-GB $f.DxCache } else { '?' }
    $opened = Open-NvidiaPanel
    $where = if ($opened) { 'Le Panneau de configuration NVIDIA vient de s''ouvrir.' } else { 'Ouvrez le Panneau de configuration NVIDIA (clic droit sur le bureau, « Afficher plus d''options »).' }
    $r = Show-Message ("C'est la 1re cause des lancements lents qui se répètent : par défaut le pilote NVIDIA garde 4 Go de pipelines " +
        "compilées par jeu. Une fois plein, chaque lancement recompile tout (1,5 à 2 fois plus lent).`n`n" +
        "Taille actuelle du cache (%LOCALAPPDATA%\NVIDIA\DXCache) : $size.`n`n$where`n" +
        "1. « Gérer les paramètres 3D », onglet « Paramètres globaux » ;`n" +
        "2. « Taille du cache des shaders » : « Illimitée » ;`n3. « Appliquer ».`n`n" +
        "Le lanceur ne modifie pas le pilote lui-même.`n`nAvez-vous réglé « Illimitée » ?") 'YesNo' 'Question'
    $script:S.nv_cache_ok = ($r -eq 'Yes'); Save-S
    Update-All
}
function Set-PowerMode([switch]$Auto) {
    $f = $script:Facts; $s = $script:S
    if ((Get-PowerLine) -eq 'Offline') {
        if (-not $Auto) { Show-Message "Sur batterie, Windows bride le processeur et la carte graphique.`nBranchez le chargeur avant de jouer." 'OK' 'Warning' | Out-Null }
        else { Write-UiLog 'Alimentation : le PC est sur batterie, branchez le chargeur.' }
        return 'skipped'
    }
    if ($f -and $f.SchemeGood) {
        if ($s.ancien_plan -and -not $Auto) {
            $label = Get-SchemeLabel $s.ancien_plan $s.ancien_plan
            if ((Show-Message "Revenir à l'ancien mode d'alimentation (« $label ») ?" 'YesNo' 'Question') -eq 'Yes') {
                & powercfg.exe /setactive $s.ancien_plan
                if ($LASTEXITCODE -eq 0) { Write-UiLog "Alimentation : retour au mode « $label »."; $s.ancien_plan = ''; Save-S }
                Request-Facts
            }
        }
        return 'done'
    }
    $before = if ($f) { $f.SchemeGuid } else { '' }
    & powercfg.exe /setactive SCHEME_MIN
    if ($LASTEXITCODE -eq 0) {
        if ($before -and -not $s.ancien_plan) { $s.ancien_plan = $before; Save-S }
        Write-UiLog "Alimentation : mode « Performances élevées » activé. Pour revenir à l'ancien, cliquez de nouveau sur la tuile 8 (ou : powercfg /setactive $before)."
        Request-Facts
        return 'done'
    }
    # Portables récents (veille moderne) : pas de mode « Performances élevées », seulement le curseur de Windows.
    Write-UiLog 'Alimentation : ce PC n''a pas le mode « Performances élevées » ; réglez « Mode d''alimentation : Performances optimales » dans Paramètres.'
    if (-not $Auto) {
        Start-Process 'ms-settings:powersleep'
        Show-Message "Ce PC n'a pas le mode « Performances élevées ».`nDans Paramètres > Système > Alimentation, choisissez « Mode d'alimentation : Performances optimales »." | Out-Null
    }
    'skipped'
}
function Select-DlssFolder {
    $r = Show-Message ("Le DLSS a besoin des DLL de Streamline de NVIDIA (sl.interposer.dll, sl.common.dll, sl.dlss.dll, nvngx_dlss.dll), " +
        "à télécharger vous-même (licence NVIDIA, elles ne sont pas fournies ici) : dossier bin\x64 de l'archive sur`n" +
        "https://github.com/NVIDIA-RTX/Streamline/releases`n`n" +
        "Oui : choisir maintenant le dossier qui les contient.`nNon : ouvrir la page de téléchargement.") 'YesNoCancel' 'Question'
    if ($r -eq 'No') { Start-Process 'https://github.com/NVIDIA-RTX/Streamline/releases'; return }
    if ($r -ne 'Yes') { return }
    $dlg = New-Object System.Windows.Forms.FolderBrowserDialog
    $dlg.Description = 'Dossier qui contient sl.interposer.dll (Streamline, bin\x64)'
    if ($script:S.dlss_dir -and (Test-Path $script:S.dlss_dir)) { $dlg.SelectedPath = $script:S.dlss_dir }
    if ($dlg.ShowDialog($script:form) -ne 'OK') { return }
    $p = $dlg.SelectedPath
    if (-not (Test-Path (Join-Path $p 'sl.interposer.dll'))) { Show-Message 'sl.interposer.dll introuvable dans ce dossier.' 'OK' 'Warning' | Out-Null; return }
    $missing = @('sl.common.dll', 'sl.dlss.dll', 'nvngx_dlss.dll') | Where-Object { -not (Test-Path (Join-Path $p $_)) }
    if ($missing) { Show-Message "Attention, il manque : $($missing -join ', '). Sans elles le jeu revient à son upscaler." 'OK' 'Warning' | Out-Null }
    $script:S.dlss_dir = $p; Save-S
    Write-UiLog "DLSS : dossier Streamline $p"
    Update-All
}
function Show-GpuInfo {
    $f = $script:Facts
    $state = (Get-CheckState 'gpu').Text
    if ($f -and $f.GpuName -match 'RTX') {
        $r = Show-Message ("Carte : $state.`n`nLe DLSS est possible sur cette carte RTX : il remplace l'upscaler du jeu " +
            "(mesuré sur RTX 3050 : ultra performance 90 ms par image, image propre). La génération d'images (Frame Generation) " +
            "demande une RTX 40 ou 50 ; ce lanceur ne la propose pas.`n`nRégler le dossier du DLSS maintenant ?") 'YesNo' 'Question'
        if ($r -eq 'Yes') { Select-DlssFolder }
    } else {
        Show-Message ("Carte : $state.`n`nLe DLSS demande une carte NVIDIA RTX ; sans elle, les réglages « Recommandé » " +
            "(540p + upscaler du jeu) sont les bons.") | Out-Null
    }
}

# ---------------------------------------------------------------- « Tout préparer pour moi »
$AutoOrder = @('rust', 'vs', 'build', 'game', 'shaders', 'pipes', 'power')
function Start-AutoSetup {
    if ($script:Job -or $script:Queue) { return }
    $todo = @($AutoOrder | Where-Object { (Get-CheckState $_).Auto } | ForEach-Object { "  - $($CheckById[$_].Title)" })
    if (-not $todo) { Show-Message "Tout ce que le lanceur sait faire seul est déjà prêt.`nRegardez les tuiles orange ou rouges restantes : elles demandent une action de votre part." | Out-Null; return }
    $r = Show-Message ("Le lanceur va faire, dans l'ordre :`n$($todo -join "`n")`n`n" +
        "Les installations ouvrent une fenêtre où Windows vous demande l'autorisation. La traduction des shaders prend 15 à 30 minutes. " +
        "Le mode d'alimentation passe en « Performances élevées » (réversible, tuile 8).`n`nCommencer ?") 'YesNo' 'Question'
    if ($r -ne 'Yes') { return }
    $script:Queue = New-Object System.Collections.Generic.List[string]
    foreach ($id in $AutoOrder) { $script:Queue.Add($id) }
    Write-UiLog '=== Tout préparer pour moi : début ==='
    Set-Busy
    Request-Facts { Step-Queue }
}
function Step-Queue {
    if (-not $script:Queue) { return }
    while ($script:Queue.Count -gt 0) {
        $id = $script:Queue[0]; $script:Queue.RemoveAt(0)
        if (-not (Get-CheckState $id).Auto) { continue }
        Write-UiLog "--- Étape $($CheckById[$id].N) : $($CheckById[$id].Title)"
        $r = Invoke-Fix $id -Auto
        if ($r -eq 'started') { return }
        if ($r -eq 'failed') { Stop-Queue "Étape « $($CheckById[$id].Title) » impossible."; return }
    }
    Complete-Queue
}
function Resume-Queue([bool]$ok) {
    if (-not $script:Queue) { Request-Facts; return }
    if (-not $ok) { Stop-Queue 'La dernière étape n''a pas réussi.'; return }
    Request-Facts { Step-Queue }
}
function Stop-Queue([string]$why) {
    if (-not $script:Queue) { return }
    $script:Queue = $null
    Write-UiLog "=== Préparation interrompue : $why ==="
    Set-Busy; Update-All
    Show-Message "Préparation interrompue : $why`nLe détail est dans le journal en bas de la fenêtre. Corrigez puis relancez « Tout préparer pour moi »." 'OK' 'Warning' | Out-Null
}
function Complete-Queue {
    $script:Queue = $null
    Set-Busy; Update-All
    Write-UiLog '=== Tout préparer pour moi : terminé ==='
    $f = $script:Facts; $s = $script:S
    $manual = @()
    if ($f -and $f.Nvidia -and -not $s.nv_cache_ok) { $manual += '- Cache de shaders NVIDIA sur « Illimitée » (tuile 7) : 1re cause des lancements lents.' }
    if ((Get-PowerLine) -eq 'Offline') { $manual += '- Branchez le chargeur (tuile 8).' }
    if ((Get-CheckState 'pagefile').Level -eq 'warn') { $manual += '- Mémoire virtuelle gérée par Windows (tuile 9, droits d''administrateur).' }
    if ((Get-CheckState 'rebar').Level -eq 'warn') { $manual += '- Resizable BAR désactivé : voir le BIOS (tuile 10).' }
    if ([int]$s.parties -le 0) { $manual += '- Jouez une première partie, puis « Construire les pipelines » (tuile 6).' }
    $msg = 'Préparation terminée.'
    if ($manual) { $msg += "`n`nReste à faire par vous :`n" + ($manual -join "`n") }
    Show-Message $msg | Out-Null
    if ($f -and $f.Nvidia -and -not $s.nv_cache_ok) {
        if ((Show-Message 'Régler maintenant le cache de shaders NVIDIA ?' 'YesNo' 'Question') -eq 'Yes') { Show-NvidiaCacheGuide }
    }
    if ((Test-Path $Exe) -and (Test-Game)) { Show-Page 'play' }
}

# ---------------------------------------------------------------- jouer
function Start-Play {
    $s = $script:S
    if ($script:Job -or $script:Queue) { Show-Message 'Une tâche est en cours : attendez qu''elle finisse.' | Out-Null; return }
    if (-not (Test-Path $Exe)) { Show-Message 'VIctory n''est pas encore compilé : onglet Installation.' 'OK' 'Warning' | Out-Null; Show-Page 'prep'; return }
    if (-not (Test-Game)) { Show-Message 'Indiquez d''abord où est le jeu.' 'OK' 'Warning' | Out-Null; if (-not (Select-GameFile)) { return } }
    if (-not $s.shaders_ready) {
        $r = Show-Message ("Les shaders ne sont pas encore traduits : les premières minutes seront lentes (écrans noirs, à-coups).`n`n" +
            "Jouer quand même ? (Non : aller à l'onglet Installation)") 'YesNo' 'Warning'
        if ($r -ne 'Yes') { Show-Page 'prep'; return }
    }
    Set-GameEnv $s
    $log = Join-Path $Logs ("partie_{0:yyyy-MM-dd_HHmm}.log" -f (Get-Date))
    $script:LastGameLog = $log
    $script:lblPlayState.Text = 'Lancement du jeu…'
    $ok = Start-CmdJob -Title 'Partie' -Command "`"$Exe`" run `"$($s.game_exe)`" --console series-s --execute" -Log $log -Game -OnExit {
        param($code, $j)
        $script:S.parties = [int]$script:S.parties + 1; Save-S
        $script:lblPlayState.Text = "Partie terminée (code $code)."
        Update-All
    }
    if (-not $ok) { $script:lblPlayState.Text = 'Le jeu n''a pas pu être lancé : voir le journal.' }
}
function Open-LastGameLog {
    $log = $script:LastGameLog
    if (-not ($log -and (Test-Path $log))) { $log = Get-ChildItem $Logs -Filter 'partie_*.log' -ErrorAction SilentlyContinue | Sort-Object LastWriteTime | Select-Object -Last 1 -ExpandProperty FullName }
    if ($log) { Start-Process notepad.exe "`"$log`"" } else { Show-Message 'Aucune partie lancée pour le moment.' | Out-Null }
}
function Backup-Saves {
    $data = Join-Path $env:LOCALAPPDATA 'victory\data'
    if (-not (Test-Path $data)) { Show-Message 'Aucune sauvegarde trouvée pour le moment.' | Out-Null; return }
    $dst = Join-Path $Home_ ("sauvegardes\{0:yyyy-MM-dd_HHmmss}" -f (Get-Date))
    Copy-Item -Recurse $data $dst
    Write-UiLog "Parties copiées dans $dst"
    Show-Message "Parties copiées dans`n$dst" | Out-Null
}

# ---------------------------------------------------------------- fenêtre
$form = New-Object System.Windows.Forms.Form
$script:form = $form
$form.Text = $Title_
$form.Size = New-Object System.Drawing.Size(1120, 790)
$form.MinimumSize = New-Object System.Drawing.Size(1000, 720)
$form.StartPosition = 'CenterScreen'
$form.BackColor = $Pal.Body
$form.Font = New-Font 9.5
try { $form.Icon = [System.Drawing.Icon]::ExtractAssociatedIcon((Join-Path $PSHOME 'powershell.exe')) } catch { }

function New-Label([string]$text, $font, $color, [int]$x, [int]$y, [int]$w, [int]$h) {
    $l = New-Object System.Windows.Forms.Label
    $l.Text = $text; $l.Font = $font; $l.ForeColor = $color; $l.BackColor = [System.Drawing.Color]::Transparent
    $l.SetBounds($x, $y, $w, $h)
    $l
}
function New-FlatButton([string]$text, $back, $fore, [int]$x, [int]$y, [int]$w, [int]$h, $font) {
    $b = New-Object System.Windows.Forms.Button
    $b.Text = $text; $b.FlatStyle = 'Flat'; $b.BackColor = $back; $b.ForeColor = $fore
    $b.FlatAppearance.BorderColor = $Pal.Accent; $b.FlatAppearance.BorderSize = 1
    $b.Cursor = 'Hand'; $b.SetBounds($x, $y, $w, $h)
    if ($font) { $b.Font = $font }
    $b
}

# En-tête : nom, sous-titre, onglets
$header = New-Object System.Windows.Forms.Panel
$header.Dock = 'Top'; $header.Height = 76; $header.BackColor = $Pal.Head
$logoMark = New-Object System.Windows.Forms.Panel; $logoMark.SetBounds(22, 18, 10, 40); $logoMark.BackColor = $Pal.Orchid
$logo = New-Label 'Sagouindupm | VIctory' (New-Font 20 'Bold') ([System.Drawing.Color]::White) 40 8 420 40
$sub = New-Label 'Profil Series S · optimisé pour les petites configs' (New-Font 9.5) $Pal.Orchid 43 47 420 20
$tabs = New-Object System.Windows.Forms.FlowLayoutPanel
$tabs.Dock = 'Right'; $tabs.Width = 3 * 150; $tabs.BackColor = $Pal.Head; $tabs.WrapContents = $false
$script:TabButtons = @{}
foreach ($t in @(@('play', 'Jouer'), @('prep', 'Installation'), @('help', 'Aide'))) {
    $b = New-Object System.Windows.Forms.Button
    $b.Text = $t[1]; $b.Tag = $t[0]; $b.Size = New-Object System.Drawing.Size(150, 76); $b.Margin = New-Object System.Windows.Forms.Padding 0
    $b.FlatStyle = 'Flat'; $b.FlatAppearance.BorderSize = 0; $b.FlatAppearance.MouseOverBackColor = $Pal.AccentHover
    $b.BackColor = $Pal.Head; $b.ForeColor = [System.Drawing.Color]::White; $b.Font = New-Font 11.5 'Bold'; $b.Cursor = 'Hand'
    $b.Add_Click({ Show-Page $this.Tag })
    $tabs.Controls.Add($b); $script:TabButtons[$t[0]] = $b
}
$header.Controls.AddRange(@($tabs, $logoMark, $logo, $sub))

# Bas : tâche en cours, Arrêter, journal
$bottom = New-Object System.Windows.Forms.Panel
$bottom.Dock = 'Bottom'; $bottom.Height = 150; $bottom.BackColor = $Pal.Card; $bottom.Padding = New-Object System.Windows.Forms.Padding(12, 6, 12, 10)
$txtLog = New-Object System.Windows.Forms.TextBox
$txtLog.Multiline = $true; $txtLog.ReadOnly = $true; $txtLog.ScrollBars = 'Vertical'; $txtLog.Dock = 'Fill'; $txtLog.WordWrap = $true
$txtLog.BackColor = $Pal.LogBg; $txtLog.ForeColor = $Pal.LogFg; $txtLog.Font = New-Font 9 'Regular' 'Consolas'; $txtLog.BorderStyle = 'None'
$script:txtLog = $txtLog
$taskBar = New-Object System.Windows.Forms.Panel; $taskBar.Dock = 'Top'; $taskBar.Height = 34
# Docké (pas ancré) : le panneau est redimensionné par son propre Dock après l'ajout des enfants.
$lblTask = New-Label '' (New-Font 9.5 'Bold') $Pal.Accent 0 0 10 10
$lblTask.Dock = 'Fill'; $lblTask.TextAlign = 'MiddleLeft'; $lblTask.AutoEllipsis = $true
$script:lblTask = $lblTask
$taskButtons = New-Object System.Windows.Forms.FlowLayoutPanel; $taskButtons.Dock = 'Right'; $taskButtons.Width = 340; $taskButtons.FlowDirection = 'RightToLeft'; $taskButtons.WrapContents = $false
$btnLogs = New-FlatButton 'Dossier des journaux' $Pal.Card $Pal.Accent 0 0 200 28 $null
$btnLogs.Add_Click({ Start-Process explorer.exe "`"$Logs`"" })
$btnStop = New-FlatButton 'Arrêter' $Pal.BadBg $Pal.BadFg 0 0 100 28 $null
$btnStop.Visible = $false; $btnStop.Add_Click({ Stop-Job })
$taskButtons.Controls.AddRange(@($btnLogs, $btnStop))
$taskBar.Controls.Add($lblTask); $taskBar.Controls.Add($taskButtons)
$bottom.Controls.Add($txtLog); $bottom.Controls.Add($taskBar)

$content = New-Object System.Windows.Forms.Panel; $content.Dock = 'Fill'; $content.BackColor = $Pal.Body

# ---- page Jouer
$pgPlay = New-Object System.Windows.Forms.Panel; $pgPlay.Dock = 'Fill'; $pgPlay.AutoScroll = $true
$card = New-Object System.Windows.Forms.Panel; $card.SetBounds(24, 18, 640, 470); $card.BackColor = $Pal.Card
$h1 = New-Font 12.5 'Bold'
$card.Controls.Add((New-Label 'Votre jeu' $h1 $Pal.Accent 20 14 400 26))
$lblGame = New-Label '' (New-Font 9.5) $Pal.Text 20 44 600 22; $lblGame.AutoEllipsis = $true
$btnGame = New-FlatButton 'Changer…' $Pal.Card $Pal.Accent 20 72 120 30 $null; $btnGame.Add_Click({ [void](Select-GameFile) })
$btnFind = New-FlatButton 'Chercher automatiquement' $Pal.Card $Pal.Accent 150 72 210 30 $null
$btnFind.Add_Click({ if (-not $script:Job) { Start-GameSearch } })
$card.Controls.AddRange(@($lblGame, $btnGame, $btnFind))
$card.Controls.Add((New-Label "Mode d'image" $h1 $Pal.Accent 20 118 400 26))
$rbRec = New-Object System.Windows.Forms.RadioButton; $rbRec.Text = 'Recommandé : nos réglages Series S mesurés (540p + upscaler du jeu)'; $rbRec.Tag = 'off'
$rbUltra = New-Object System.Windows.Forms.RadioButton; $rbUltra.Text = 'DLSS ultra performance (le plus rapide, conseillé sur 6-8 Go)'; $rbUltra.Tag = 'ultra'
$rbPerf = New-Object System.Windows.Forms.RadioButton; $rbPerf.Text = 'DLSS performance'; $rbPerf.Tag = 'performance'
$rbQual = New-Object System.Windows.Forms.RadioButton; $rbQual.Text = 'DLSS qualité (le plus beau, plus lent)'; $rbQual.Tag = 'quality'
$script:ModeButtons = @($rbRec, $rbUltra, $rbPerf, $rbQual)
$y = 148
foreach ($rb in $script:ModeButtons) {
    $rb.SetBounds(24, $y, 600, 24); $y += 26
    $rb.Add_CheckedChanged({ if ($this.Checked -and -not $script:Updating) { $script:S.dlss_mode = $this.Tag; Save-S; Write-UiLog "Mode d'image : $($this.Text)" } })
    $card.Controls.Add($rb)
}
$lnkDlss = New-Object System.Windows.Forms.LinkLabel; $lnkDlss.SetBounds(24, 254, 600, 22); $lnkDlss.LinkColor = $Pal.Accent
$lnkDlss.Add_LinkClicked({ Select-DlssFolder })
$card.Controls.Add($lnkDlss)
$card.Controls.Add((New-Label 'Préparation' $h1 $Pal.Accent 20 292 400 26))
$lblShaders = New-Label '' (New-Font 10 'Bold') $Pal.Text 24 322 600 22
$lblPipes = New-Label '' (New-Font 10 'Bold') $Pal.Text 24 348 600 22
$lblPrepWarn = New-Label '' (New-Font 9.5) $Pal.WarnFg 24 378 600 60
$btnGoPrep = New-FlatButton 'Aller à l''installation' $Pal.Card $Pal.Accent 24 430 200 28 $null; $btnGoPrep.Add_Click({ Show-Page 'prep' })
$card.Controls.AddRange(@($lblShaders, $lblPipes, $lblPrepWarn, $btnGoPrep))

$btnPlay = New-Object System.Windows.Forms.Button
$btnPlay.Text = 'JOUER'; $btnPlay.SetBounds(690, 18, 380, 150); $btnPlay.Font = New-Font 34 'Bold'
$btnPlay.FlatStyle = 'Flat'; $btnPlay.FlatAppearance.BorderSize = 0; $btnPlay.BackColor = $Pal.Accent; $btnPlay.ForeColor = [System.Drawing.Color]::White
$btnPlay.FlatAppearance.MouseOverBackColor = $Pal.AccentHover; $btnPlay.Cursor = 'Hand'
$btnPlay.Add_Click({ try { Start-Play } catch { Write-UiLog "Erreur : $($_.Exception.Message)" } })
$lblPlayState = New-Label 'Prêt.' (New-Font 11 'Bold') $Pal.Text 692 178 380 48
$script:lblPlayState = $lblPlayState
$btnOpenLog = New-FlatButton 'Ouvrir le journal de la dernière partie' $Pal.Card $Pal.Accent 690 232 380 34 $null
$btnOpenLog.Add_Click({ Open-LastGameLog })
$lblHint = New-Label '' (New-Font 9.5) $Pal.Text 692 280 380 64
$btnHintPipes = New-FlatButton 'Construire les pipelines maintenant' $Pal.Accent ([System.Drawing.Color]::White) 690 348 380 36 (New-Font 10 'Bold')
$btnHintPipes.Add_Click({ try { [void](Invoke-Fix 'pipes') } catch { Write-UiLog "Erreur : $($_.Exception.Message)" } })
$pgPlay.Controls.AddRange(@($card, $btnPlay, $lblPlayState, $btnOpenLog, $lblHint, $btnHintPipes))

# ---- page Installation : en haut le résumé et « Tout préparer pour moi », puis 12 tuiles
$pgPrep = New-Object System.Windows.Forms.Panel; $pgPrep.Dock = 'Fill'
$prepTop = New-Object System.Windows.Forms.Panel; $prepTop.Dock = 'Top'; $prepTop.Height = 92
$lblSummary = New-Label 'Vérification de votre PC…' (New-Font 15 'Bold') $Pal.Text 24 10 520 32
$bar = New-Object System.Windows.Forms.ProgressBar; $bar.SetBounds(26, 46, 400, 14); $bar.Maximum = 100
$lblLegend = New-Label '✓ prêt    • à vérifier    ✗ à faire    i information        Cliquez sur une tuile pour agir.' (New-Font 9) $Pal.Muted 24 66 620 20
$prepButtons = New-Object System.Windows.Forms.FlowLayoutPanel; $prepButtons.Dock = 'Right'; $prepButtons.Width = 420
$prepButtons.FlowDirection = 'RightToLeft'; $prepButtons.WrapContents = $false; $prepButtons.Padding = New-Object System.Windows.Forms.Padding(0, 14, 14, 0)
$btnRecheck = New-FlatButton 'Revérifier' $Pal.Card $Pal.Accent 0 0 120 54 $null
$btnRecheck.Add_Click({ Request-Facts })
$btnAuto = New-FlatButton 'Tout préparer pour moi' $Pal.Accent ([System.Drawing.Color]::White) 0 0 250 54 (New-Font 12 'Bold')
$btnAuto.FlatAppearance.BorderSize = 0; $btnAuto.FlatAppearance.MouseOverBackColor = $Pal.AccentHover
$btnAuto.Add_Click({ try { Start-AutoSetup } catch { Write-UiLog "Erreur : $($_.Exception.Message)" } })
$prepButtons.Controls.AddRange(@($btnRecheck, $btnAuto))
$prepTop.Controls.AddRange(@($lblSummary, $bar, $lblLegend, $prepButtons))

$grid = New-Object System.Windows.Forms.TableLayoutPanel
$grid.Dock = 'Fill'; $grid.ColumnCount = 4; $grid.RowCount = 3; $grid.Padding = New-Object System.Windows.Forms.Padding(14, 0, 14, 8)
for ($i = 0; $i -lt 4; $i++) { [void]$grid.ColumnStyles.Add((New-Object System.Windows.Forms.ColumnStyle('Percent', 25))) }
for ($i = 0; $i -lt 3; $i++) { [void]$grid.RowStyles.Add((New-Object System.Windows.Forms.RowStyle('Percent', 33.33))) }
$script:Tiles = @{}
$script:TileColor = @{}
$TileClick = { try { [void](Invoke-Fix $this.Tag) } catch { Write-UiLog "Erreur : $($_.Exception.Message)" } }
foreach ($chk in $Checks) {
    $tile = New-Object System.Windows.Forms.Panel
    $tile.Dock = 'Fill'; $tile.Margin = New-Object System.Windows.Forms.Padding 6; $tile.Padding = New-Object System.Windows.Forms.Padding(16, 8, 10, 6)
    $tile.Tag = $chk.Id; $tile.Cursor = 'Hand'
    # Bande de couleur à gauche de la tuile
    $tile.Add_Paint({
            param($sender, $e)
            $col = $script:TileColor[[string]$sender.Tag]
            if ($col) { $br = New-Object System.Drawing.SolidBrush $col; $e.Graphics.FillRectangle($br, 0, 0, 6, $sender.Height); $br.Dispose() }
        })
    $lHead = New-Label "$($chk.N)  $($chk.Title)" (New-Font 10.5 'Bold') $Pal.Text 0 0 10 24; $lHead.Dock = 'Top'
    $lState = New-Label '' (New-Font 9.5 'Bold') $Pal.Muted 0 0 10 22; $lState.Dock = 'Top'; $lState.AutoEllipsis = $true
    $lDesc = New-Label $chk.Desc (New-Font 8.75) $Pal.Muted 0 0 10 10; $lDesc.Dock = 'Fill'
    $lAct = New-Label '' (New-Font 9 'Bold') $Pal.Accent 0 0 10 20; $lAct.Dock = 'Bottom'
    # Ordre d'ajout : le dernier ajouté est placé en premier (le titre tout en haut, la description remplit le reste).
    $tile.Controls.Add($lDesc); $tile.Controls.Add($lAct); $tile.Controls.Add($lState); $tile.Controls.Add($lHead)
    foreach ($ctl in @($tile, $lHead, $lState, $lDesc, $lAct)) { $ctl.Tag = $chk.Id; $ctl.Cursor = 'Hand'; $ctl.Add_Click($TileClick) }
    $grid.Controls.Add($tile)
    $script:Tiles[$chk.Id] = @{ Tile = $tile; State = $lState; Action = $lAct }
}
$pgPrep.Controls.Add($grid); $pgPrep.Controls.Add($prepTop)

# ---- page Aide
$pgHelp = New-Object System.Windows.Forms.Panel; $pgHelp.Dock = 'Fill'; $pgHelp.Padding = New-Object System.Windows.Forms.Padding(24, 16, 24, 8)
$helpButtons = New-Object System.Windows.Forms.FlowLayoutPanel; $helpButtons.Dock = 'Bottom'; $helpButtons.Height = 44
$hb1 = New-FlatButton 'Dossier des journaux' $Pal.Card $Pal.Accent 0 0 190 32 $null; $hb1.Add_Click({ Start-Process explorer.exe "`"$Logs`"" })
$hb2 = New-FlatButton 'Copier mes sauvegardes' $Pal.Card $Pal.Accent 0 0 200 32 $null; $hb2.Add_Click({ Backup-Saves })
$hb3 = New-FlatButton 'Lanceur console (avancé)' $Pal.Card $Pal.Accent 0 0 210 32 $null; $hb3.Add_Click({ Start-Process (Join-Path $PSScriptRoot 'Sagouindupm.bat') })
$hb4 = New-FlatButton 'Page du projet' $Pal.Card $Pal.Accent 0 0 150 32 $null; $hb4.Add_Click({ Start-Process 'https://github.com/sagouindupm/Mandy' })
$helpButtons.Controls.AddRange(@($hb1, $hb2, $hb3, $hb4))
$rtb = New-Object System.Windows.Forms.RichTextBox
$rtb.Dock = 'Fill'; $rtb.ReadOnly = $true; $rtb.BorderStyle = 'None'; $rtb.BackColor = $Pal.Body; $rtb.DetectUrls = $true
$rtb.Add_LinkClicked({ param($sender, $e) Start-Process $e.LinkText })
function Add-Help([string]$text, [string]$kind = 'p') {
    $rtb.SelectionStart = $rtb.TextLength
    switch ($kind) {
        'h' { $rtb.SelectionFont = New-Font 13 'Bold'; $rtb.SelectionColor = $Pal.Accent; $rtb.AppendText("$text`n") }
        default { $rtb.SelectionFont = New-Font 10; $rtb.SelectionColor = $Pal.Text; $rtb.AppendText("$text`n`n") }
    }
}
Add-Help 'Bien démarrer, dans l''ordre' 'h'
Add-Help ("1. Onglet Installation, « Tout préparer pour moi ». Le lanceur installe ce qui manque (Rust, outils C++ de Microsoft : " +
    "Windows vous demande l'autorisation), compile VIctory, trouve le jeu et traduit les shaders (15 à 30 minutes, une seule fois).`n" +
    "2. Onglet Jouer, JOUER. Pendant cette première partie, le jeu note les pipelines qu'il crée.`n" +
    "3. Après la partie : « Construire les pipelines » (quelques minutes). Les parties suivantes n'ont plus d'à-coups ni de dessins " +
    "manquants : c'est ce qui fait passer de ~5 à ~10 images par seconde sur la machine de référence (RTX 3050, i5-12400F).")
Add-Help 'Pourquoi j''ai moins d''images par seconde que prévu ?' 'h'
Add-Help ("- Shaders pas traduits ou pipelines pas construites (tuiles 5 et 6).`n" +
    "- Cache de shaders NVIDIA limité à 4 Go : une fois plein, chaque lancement recompile tout. C'est la 1re cause des lancements " +
    "lents qui se répètent : réglez « Illimitée » (tuile 7).`n" +
    "- PC portable sur batterie ou en mode économie d'énergie (tuile 8).`n" +
    "- Mémoire virtuelle de taille fixe trop petite (tuile 9).`n" +
    "- Navigateur, Discord et autres programmes ouverts, surtout avec moins de 24 Go de RAM (tuile 11).")
Add-Help 'Les tuiles' 'h'
Add-Help ("Vert ✓ : prêt. Orange • : à vérifier ou conseil. Rouge ✗ : à faire. Bleu i : information. Cliquez sur une tuile pour " +
    "la réparer ou en savoir plus. « Tout préparer pour moi » ne fait que les tuiles 1 à 6 et 8 ; le cache NVIDIA, la mémoire virtuelle " +
    "et le BIOS restent à votre main (le lanceur explique comment).")
Add-Help 'Mode d''image' 'h'
Add-Help ("« Recommandé » : nos réglages Series S mesurés (540p et l'upscaler du jeu). Sur une carte RTX, le DLSS peut le remplacer : " +
    "il faut les DLL de Streamline de NVIDIA, à télécharger vous-même (https://github.com/NVIDIA-RTX/Streamline/releases, dossier bin\x64), " +
    "puis « Activer le DLSS » dans l'onglet Jouer. Ultra performance est le plus rapide.")
Add-Help 'Où sont les fichiers' 'h'
Add-Help ("Réglages du lanceur : $Conf`nJournaux (parties, compilation, shaders, pipelines) : $Logs`n" +
    "Sauvegardes du jeu : $(Join-Path $env:LOCALAPPDATA 'victory\data')")
Add-Help 'Utilisateurs avancés' 'h'
Add-Help ("Le lanceur console series-s\Sagouindupm.bat fait la même chose en mode texte, avec les mêmes réglages " +
    "(series-s\reglages.ps1). Les scripts de mesure (scripts\bench) appellent victory.exe directement.")
Add-Help ("Ce projet ne contient aucun fichier du jeu : il faut votre propre copie. Projet sans lien avec l'éditeur ou le développeur du jeu.")
$rtb.SelectionStart = 0; $rtb.ScrollToCaret()
$pgHelp.Controls.Add($rtb); $pgHelp.Controls.Add($helpButtons)

$content.Controls.AddRange(@($pgPlay, $pgPrep, $pgHelp))
$form.Controls.Add($content); $form.Controls.Add($bottom); $form.Controls.Add($header)
$content.BringToFront()

$script:Pages = @{ play = $pgPlay; prep = $pgPrep; help = $pgHelp }
function Show-Page([string]$name) {
    foreach ($k in $script:Pages.Keys) {
        $script:Pages[$k].Visible = ($k -eq $name)
        $script:TabButtons[$k].BackColor = if ($k -eq $name) { $Pal.Accent } else { $Pal.Head }
    }
    if ($name -eq 'prep' -and -not $script:Job) { Request-Facts }
}

# ---------------------------------------------------------------- mise à jour de l'affichage
function Set-Busy {
    $busy = [bool]($script:Job -or $script:Queue -or $script:SearchTask)
    $btnStop.Visible = [bool]$script:Job
    $btnAuto.Enabled = -not $busy
    $btnPlay.Enabled = -not $busy
    $btnPlay.BackColor = if ($busy) { $Pal.Muted } else { $Pal.Accent }
    $btnHintPipes.Enabled = -not $busy
    if ($script:Job -and $script:Job.Game) { $btnPlay.Text = 'EN JEU' } else { $btnPlay.Text = 'JOUER' }
}
function Update-All {
    $s = $script:S
    $ok = 0; $total = 0
    foreach ($chk in $Checks) {
        $st = Get-CheckState $chk.Id
        $t = $script:Tiles[$chk.Id]
        $bg, $fg, $glyph = switch ($st.Level) {
            'ok' { $Pal.OkBg, $Pal.OkFg, '✓' }
            'warn' { $Pal.WarnBg, $Pal.WarnFg, '•' }
            'bad' { $Pal.BadBg, $Pal.BadFg, '✗' }
            'info' { $Pal.InfoBg, $Pal.InfoFg, 'i' }
            default { $Pal.WaitBg, $Pal.WaitFg, '…' }
        }
        $t.Tile.BackColor = $bg
        $t.State.ForeColor = $fg
        $t.State.Text = "$glyph  $($st.Text)"
        $t.Action.Text = if ($st.Action) { "$($st.Action)  →" } else { '' }
        $script:TileColor[$chk.Id] = $fg
        $t.Tile.Invalidate()
        if ($st.Level -in 'ok', 'warn', 'bad') { $total++; if ($st.Level -eq 'ok') { $ok++ } }
    }
    if ($script:FactsTask -and -not $script:Facts) { $lblSummary.Text = 'Vérification de votre PC…' }
    else { $lblSummary.Text = "Votre PC : $ok sur $total points prêts" }
    $bar.Value = if ($total) { [int](100 * $ok / $total) } else { 0 }

    # Page Jouer
    $lblGame.Text = if (Test-Game) { $s.game_exe } else { 'Pas encore indiqué : « Chercher automatiquement » ou « Changer… »' }
    $hasDlss = Test-DlssDir
    $script:Updating = $true
    foreach ($rb in $script:ModeButtons) { if ($rb.Tag -ne 'off') { $rb.Enabled = $hasDlss } }
    $mode = if ($hasDlss -and $s.dlss_mode -in 'ultra', 'performance', 'quality') { $s.dlss_mode } else { 'off' }
    foreach ($rb in $script:ModeButtons) { $rb.Checked = ($rb.Tag -eq $mode) }
    $script:Updating = $false
    $lnkDlss.Text = if ($hasDlss) { "DLSS : $($s.dlss_dir)  (changer)" } else { 'Activer le DLSS (cartes NVIDIA RTX) : choisir le dossier Streamline…' }
    $sh = Get-CheckState 'shaders'; $pp = Get-CheckState 'pipes'
    $lblShaders.Text = if ($s.shaders_ready) { '✓  Shaders traduits' } else { '✗  Shaders pas encore traduits' }
    $lblShaders.ForeColor = if ($s.shaders_ready) { $Pal.OkFg } else { $Pal.BadFg }
    $lblPipes.Text = "$(switch ($pp.Level) { 'ok' { '✓' } 'bad' { '✗' } default { '•' } })  Pipelines : $($pp.Text.ToLower())"
    $lblPipes.ForeColor = switch ($pp.Level) { 'ok' { $Pal.OkFg } 'bad' { $Pal.BadFg } default { $Pal.WarnFg } }
    $warn = @()
    if (-not (Test-Path $Exe)) { $warn += 'VIctory n''est pas encore compilé.' }
    if (-not $s.shaders_ready) { $warn += 'Sans shaders traduits, les premières minutes sont lentes (écrans noirs, à-coups).' }
    $nv = Get-CheckState 'nvcache'
    if ($nv.Level -in 'warn', 'bad') { $warn += 'Cache de shaders NVIDIA pas encore réglé sur « Illimitée » (tuile 7).' }
    $lblPrepWarn.Text = $warn -join "`n"
    $btnGoPrep.Visible = [bool]$warn
    $needPipes = ([int]$s.parties -gt 0) -and ($pp.Level -ne 'ok')
    $lblHint.Text = if ($needPipes) { 'Après une partie, « Construire les pipelines » prépare hors jeu ce que le jeu a noté : la partie suivante est plus fluide (~5 à ~10 images/s ici).' } else { '' }
    $btnHintPipes.Visible = $needPipes
    Set-Busy
}

# ---------------------------------------------------------------- minuterie : runspaces et tâche en cours
$timer = New-Object System.Windows.Forms.Timer
$timer.Interval = 400
$timer.Add_Tick({
        try {
            if ($script:FactsTask -and $script:FactsTask.Handle.IsCompleted) {
                $t = $script:FactsTask; $script:FactsTask = $null
                $r = Receive-Runspace $t
                if ($r -is [hashtable]) { $script:Facts = $r }
                Update-All
                $cbs = $script:FactsCallbacks; $script:FactsCallbacks = @()
                foreach ($cb in $cbs) { & $cb }
            }
            if ($script:SearchTask -and $script:SearchTask.Handle.IsCompleted) {
                $t = $script:SearchTask; $script:SearchTask = $null
                $r = Receive-Runspace $t
                Set-Busy
                Complete-GameSearch @($r)
            }
            Update-Job
        } catch { Write-UiLog "Erreur : $($_.Exception.Message)" }
    })

$form.Add_FormClosing({
        param($sender, $e)
        $j = $script:Job
        if (-not $j) { return }
        if ($j.Game) {
            if ((Show-Message 'Le jeu tourne encore. Fermer le lanceur quand même ? (le jeu continue)' 'YesNo' 'Question') -ne 'Yes') { $e.Cancel = $true }
            return
        }
        if ((Show-Message "« $($j.Title) » est en cours. L'arrêter et quitter ?" 'YesNo' 'Question') -eq 'Yes') {
            & taskkill.exe /PID $j.Proc.Id /T /F 2>&1 | Out-Null
        } else { $e.Cancel = $true }
    })
$form.Add_Shown({
        Write-UiLog "Lanceur graphique prêt. Réglages : $Conf"
        if (-not (Test-Path $Exe)) { Write-UiLog "victory.exe introuvable ($Exe) : onglet Installation, tuile 3." }
        Request-Facts
        $timer.Start()
    })

# Premier onglet : Installation tant que l'essentiel manque, sinon Jouer.
Update-All
if ((Test-Path $Exe) -and (Test-Game) -and $script:S.shaders_ready) { Show-Page 'play' } else { Show-Page 'prep' }
[void]$form.ShowDialog()
$timer.Stop()
$form.Dispose()
