# Sagouindupm | VIctory : lanceur pour débutant (profil Series S, cartes NVIDIA 6-12 Go, processeur 6 cœurs).
# 1. Premier lancement : demande où est le jeu, écrit la configuration.
# 2. [2] Prépare les shaders : traduit tous les shaders du jeu une fois (15-30 min).
# 3. [3] Construit les pipelines hors jeu, dès avant la première partie : importe la liste publiée
#    (series-s\pipelines\series-s-rtx.txt, sans code du jeu) avec les shaders de VOTRE copie, puis victory warm.
# 4. [1] Jouer : chaque partie note aussi les pipelines que le jeu crée (VICTORY_PIPELINE_BOOK) ; [3] les construit.
# Pas 'Stop' : sous Windows PowerShell 5.1, chaque ligne que le jeu écrit sur sa sortie d'erreur
# devient une erreur, et le lanceur s'arrêtait à la première ligne du journal.
$ErrorActionPreference = 'Continue'
# Chemins et lanceur.json : series-s\reglages.ps1, qui applique series-s\reglages.json (réglages Series S,
# ray tracing, règle des 24 Go, DLSS ; les mêmes que pour le lanceur graphique).
. (Join-Path $PSScriptRoot 'reglages.ps1')

function Show-Title {
    Clear-Host
    $Host.UI.RawUI.WindowTitle = 'Sagouindupm | VIctory'
    Write-Host ''
    Write-Host '   ███  Sagouindupm | VIctory  ███' -ForegroundColor Magenta
    Write-Host '   Profil Series S - optimisé pour les petites configs' -ForegroundColor DarkMagenta
    Write-Host ''
}

function Initialize-Game($s) {
    # Une copie complète : l'exe avec ses archives à côté (« jeu » dans reglages.json), pas l'exe seul.
    while (-not ($s.game_exe -and (Test-Path $s.game_exe) -and -not ($problem = Test-GameCopy $s.game_exe))) {
        if ($s.game_exe -and $problem) { Write-Host $problem -ForegroundColor Red; $problem = $null }
        Write-Host "Où est le jeu ?" -ForegroundColor Yellow
        Write-Host "Glissez ici le fichier game_scarlett_master.exe de VOTRE copie du jeu (complète), puis Entrée."
        $p = (Read-Host 'Chemin').Trim('"', ' ')
        if ($p -and (Test-Path $p) -and $p.EndsWith('.exe')) { $s.game_exe = $p } else { Write-Host 'Fichier introuvable, réessayez.' -ForegroundColor Red }
    }
    Write-VictoryToml $s
}

function Set-Dlss($s) {
    if (-not (Get-HardwareProfile (Get-Hardware)).Dlss) { Write-Host 'DLSS : il faut une carte NVIDIA RTX. Ici, 540p et l''upscaler du jeu.' -ForegroundColor Yellow; return }
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
    Set-GameEnv $s
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
    Set-GameEnv $s
    # Les faits de la carte (NVIDIA : vagues de 32, 1 000 000 de vues) : sans eux le cache ne sert à rien en jeu.
    # NVIDIA seulement : une carte AMD garde la traduction Series S générique.
    if ((Get-HardwareProfile (Get-Hardware)).ShadersNvidia) {
        $hostEnv = Get-ShaderHostEnv; foreach ($k in $hostEnv.Keys) { Set-Item "env:$k" $hostEnv[$k] }
    }
    # La liste partagée d'abord : la traduction garde au passage les shaders de vos fichiers qu'elle nomme ([3]).
    $list = Get-SharedPipelineList
    if ($list) { & $Exe pipelines import $list | Tee-Object -FilePath (Join-Path $Logs 'pipelines_import.txt') }
    Write-Host "`nTraduction de tous les shaders (15 à 30 minutes, le PC sera très occupé)..." -ForegroundColor Magenta
    $out = & $Exe shaders $s.shaders_dir --jobs (Get-ShaderJobs) | Tee-Object -FilePath (Join-Path $Logs 'shaders_traduction.txt')
    $code = $LASTEXITCODE
    $out | Select-Object -Last 3
    # Une copie complète en donne ~25 000 ; quelques centaines veulent dire une copie incomplète.
    $n = Get-ShadersCount $out
    if ($code -ne 0) { Write-Host "La traduction s'est arrêtée (code $code), voir $Logs." -ForegroundColor Red }
    elseif ($null -ne $n -and $n -lt [int]$Reglages.jeu.shaders_min) {
        $s.shaders_ready = $false; Save-Settings $s
        Write-Host "Seulement $n shaders traduits : une copie complète du jeu en donne ~25 000. Vérifiez la copie du jeu ([5]) et le dossier des shaders, puis recommencez." -ForegroundColor Red
    }
    else { $s.shaders_ready = $true; Save-Settings $s; Write-Host "Shaders prêts$(if ($null -ne $n) { " ($n)" }) !" -ForegroundColor Green }
}

function Initialize-Pipelines($s) {
    Set-GameEnv $s
    # La liste partagée : les pipelines dont le jeu se sert jusqu'après la mission du bateau (clés, empreintes des
    # shaders, états ; aucun code du jeu). Les shaders eux-mêmes viennent de VOTRE dossier de shaders : gardés
    # par [2] ; sinon --shaders les y cherche (~7 min ; rien à faire s'ils sont déjà là).
    $list = Get-SharedPipelineList
    if ($list) {
        Write-Host "`nImport de la liste de pipelines partagée..." -ForegroundColor Magenta
        $import = @('pipelines', 'import', $list)
        if ($s.shaders_dir -and (Test-Path $s.shaders_dir)) { $import += @('--shaders', $s.shaders_dir) }
        else { Write-Host 'Dossier de shaders inconnu : faites [2] d''abord, sinon seules les pipelines de vos parties seront construites.' -ForegroundColor Yellow }
        & $Exe @import | Tee-Object -FilePath (Join-Path $Logs 'pipelines_import.txt')
    }
    Write-Host "`nConstruction des pipelines (liste partagée et vos parties, quelques minutes)..." -ForegroundColor Magenta
    & $Exe warm --used | Tee-Object -FilePath (Join-Path $Logs 'pipelines_warm.txt') | Select-Object -Last 2
    # Repère lu par le lanceur graphique : pipelines construites à ce nombre de parties.
    # Repère de la liste partagée construite (lu par le lanceur graphique, étape 4).
    if ($LASTEXITCODE -eq 0) { $s.pipelines_parties = $s.parties; $s.pipelines_liste = Get-SharedPipelineMark; Save-Settings $s }
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
    Write-Host '(ou téléchargez la version prête : https://github.com/sagouindupm/Mandy/releases)'
    Read-Host 'Entrée pour quitter'; exit 1
}
$s = Complete-Settings (Get-Settings)
Show-Title
Initialize-Game $s
# Profil choisi d'après ce PC (« materiel » dans reglages.json), le même que dans le lanceur graphique.
$Profil = Get-HardwareProfile (Get-Hardware)
while ($true) {
    Show-Title
    $state = if ($s.shaders_ready) { 'prêts' } else { 'pas encore préparés' }
    $dlss = if ($s.dlss_mode -and $s.dlss_mode -ne 'off') { $s.dlss_mode } else { 'désactivé' }
    Write-Host "  Profil détecté : Series S · $($Profil.Label)" -ForegroundColor DarkMagenta
    foreach ($w in $Profil.Warnings) { Write-Host "  ! $w" -ForegroundColor Yellow }
    foreach ($t in $Profil.Tips) { Write-Host "  $t" -ForegroundColor DarkYellow }
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
