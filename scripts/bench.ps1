# Scored run of a title: the same input every time, and numbers that say how far it got.
# Where smoke.ps1 answers "did this pull request break the title?", this answers "did it get
# further?" – one JSON record per run, to compare changes on one machine.
#
#   .\scripts\bench.ps1 -Game 'D:\Games\SomeTitle' [-Seconds 300] [-Presses '20:A,25:A,30:A']
#       [-DeviceType lockhart] [-Label my-change] [-Out .\bench-results] [-Mandy <mandy.exe>]
#
# -Presses walks the title's menus without a person (MANDY_PRESS_AT: seconds:BUTTON, buttons
#  A B X Y UP DOWN LEFT RIGHT MENU VIEW LB RB). It presses whatever is on screen at that time,
#  licence and settings screens included: only use it on a title whose screens you have already
#  been through yourself. The default presses nothing.
# -DeviceType sets device_type in the title's mandy.toml for this run and puts the old value back.
#
# Networking is off (MANDY_NETWORK=0). Build first: cargo build --release -p mandy
param(
    [Parameter(Mandatory = $true)][string]$Game,
    [int]$Seconds = 300,
    [string]$Presses = '',
    [ValidateSet('', 'anaconda', 'lockhart')][string]$DeviceType = '',
    [string]$Label = 'run',
    [string]$Out = '',
    [string]$Mandy = ''
)
$ErrorActionPreference = 'Stop'

$here = if ($PSScriptRoot) { $PSScriptRoot } else { Split-Path -Parent $MyInvocation.MyCommand.Path }
$root = Split-Path $here -Parent
if (-not $Out) { $Out = Join-Path $root 'bench-results' }
if (-not $Mandy) { $Mandy = Join-Path $root 'target\release\mandy.exe' }
if (-not (Test-Path $Mandy)) { throw "no $Mandy - build it first (cargo build --release -p mandy)" }
$commit = (git -C $root rev-parse --short HEAD).Trim()
$dirty = [bool](git -C $root status --porcelain --untracked-files=no -- crates Cargo.toml Cargo.lock)
$started = Get-Date

# The title's data folder is only known once MANDY has run it: find mandy.toml files to edit.
$dataRoot = if ($env:MANDY_DATA_ROOT) { $env:MANDY_DATA_ROOT } else { Join-Path $env:LOCALAPPDATA 'MANDY' }
$restore = @()
if ($DeviceType) {
    $tomls = @(Get-ChildItem -Path $dataRoot -Filter mandy.toml -Recurse -Depth 1 -ErrorAction SilentlyContinue)
    if (-not $tomls) { throw "-DeviceType needs the title's mandy.toml under $dataRoot - run the title once first" }
    foreach ($t in $tomls) {
        $old = [IO.File]::ReadAllText($t.FullName)
        $new = [regex]::Replace($old, '(?m)^device_type\s*=\s*"[^"]*"', "device_type = `"$DeviceType`"")
        if ($new -ne $old) {
            $restore += , @($t.FullName, $old)
            [IO.File]::WriteAllText($t.FullName, $new, (New-Object System.Text.UTF8Encoding($false)))
        }
    }
}

$set = @{
    MANDY_NETWORK     = '0'
    MANDY_ACTIVITY    = '1'
    MANDY_RUN_SECONDS = "$Seconds"
    MANDY_FRAME_DUMP  = '120'
}
if ($Presses) { $set['MANDY_PRESS_AT'] = $Presses }
$timedOut = $false
try {
    foreach ($k in $set.Keys) { Set-Item -Path "Env:$k" -Value $set[$k] }
    # ProcessStartInfo, as in smoke.ps1 (Start-Process fails on a PATH present twice in 5.1).
    $psi = New-Object System.Diagnostics.ProcessStartInfo
    $psi.FileName = $Mandy
    $psi.Arguments = 'run "' + $Game + '"'
    $psi.UseShellExecute = $false
    $psi.CreateNoWindow = $true
    $psi.RedirectStandardOutput = $true
    $psi.RedirectStandardError = $true
    $psi.StandardOutputEncoding = [System.Text.Encoding]::UTF8
    $psi.StandardErrorEncoding = [System.Text.Encoding]::UTF8
    $p = New-Object System.Diagnostics.Process
    $p.StartInfo = $psi
    $null = $p.Start()
    $stdoutTask = $p.StandardOutput.ReadToEndAsync()
    $stderrTask = $p.StandardError.ReadToEndAsync()
    if (-not $p.WaitForExit(($Seconds + 120) * 1000)) {
        if (-not $p.HasExited) { $p.Kill() }
        $timedOut = $true
    }
    $p.WaitForExit()
    $stdout = $stdoutTask.Result
    $null = $stderrTask.Result
}
finally {
    foreach ($k in $set.Keys) { Remove-Item -Path "Env:$k" -ErrorAction SilentlyContinue }
    foreach ($r in $restore) { [IO.File]::WriteAllText($r[0], $r[1], (New-Object System.Text.UTF8Encoding($false))) }
}

$logLine = ($stdout -split "`n" | Where-Object { $_ -match '^Log:\s+(.+)$' } | Select-Object -Last 1)
if (-not $logLine) { throw "MANDY printed no log path; its output:`n$stdout" }
$log = ($logLine -replace '^Log:\s+', '').Trim()
$text = Get-Content $log -Raw -Encoding utf8
$logs = Split-Path $log -Parent

function Last-Int([string]$pattern) {
    $m = [regex]::Matches($text, $pattern)
    if ($m.Count) { [long]$m[$m.Count - 1].Groups[1].Value } else { $null }
}
function Max-Int([string]$pattern) {
    $v = [regex]::Matches($text, $pattern) | ForEach-Object { [long]$_.Groups[1].Value }
    if ($v) { ($v | Measure-Object -Maximum).Maximum } else { $null }
}

# How the run ended. The title's own fatal routine and assertions are told apart from the host's.
$end = 'ran to the end'
$endAt = $null
$endDetail = $null
$m = [regex]::Match($text, '\[\s*([\d.]+)\]\[\d+\]\[ERROR\] unhandled exception (0x[0-9a-f]+) in (\S+)')
if ($m.Success) {
    $endAt = [double]$m.Groups[1].Value
    $endDetail = "$($m.Groups[2].Value) in $($m.Groups[3].Value)"
    $end = if ($m.Groups[2].Value -eq '0x80000003') { 'title assertion' } else { 'title crash' }
}
$r = [regex]::Match($text, '\[\s*([\d.]+)\]\[\d+\]\[(?:ERROR|WARN )\][^\r\n]*device removed[^\r\n]*')
if ($r.Success -and (-not $endAt -or [double]$r.Groups[1].Value -lt $endAt)) {
    $end = 'device removed'; $endAt = [double]$r.Groups[1].Value; $endDetail = $null
}
if ($timedOut) { $end = 'killed (no exit)' }

$report = $null
$reportPath = Join-Path $logs 'report.json'
if (Test-Path $reportPath) { try { $report = Get-Content $reportPath -Raw -Encoding utf8 | ConvertFrom-Json } catch { } }
$dump = if ($report) { $report.d3d12.last_frame_dump } else { $null }
$nonDark = if ($dump -and $dump.sampled_pixels) { [math]::Round(100.0 * $dump.non_dark_samples / $dump.sampled_pixels, 1) } else { $null }

# The title's own crash context, when it wrote one during this run (Rockstar titles do).
$ctx = @{}
$temp = Join-Path (Split-Path $logs -Parent) 'temp'
$crashLog = Get-ChildItem -Path $temp -Filter *.crash.log -ErrorAction SilentlyContinue |
    Where-Object { $_.LastWriteTime -ge $started } | Sort-Object LastWriteTime | Select-Object -Last 1
if ($crashLog) {
    foreach ($line in Get-Content $crashLog.FullName) {
        if ($line -match '^(Game State|Game Running Time|Frame|Current SRL Name|Current SRL State|Peds):\s*(.*?)(\s*\[[TF]:\d+\])?$') {
            $ctx[$Matches[1]] = $Matches[2].Trim()
        }
    }
}

$draws = [regex]::Match($text, '(?s).*console code so far: (\d+) draws with translated pipelines \((\d+) skipped[^)]*\), (\d+) dispatches \((\d+) skipped\)')
$result = [ordered]@{
    label             = $Label
    when              = $started.ToString('s')
    commit            = "$commit$(if ($dirty) { '+dirty' })"
    gpu               = if ($report) { $report.d3d12.adapter } else { $null }
    device_type       = if ($DeviceType) { $DeviceType } else { 'as configured' }
    seconds_asked     = $Seconds
    presses           = $Presses
    end               = $end
    end_at_s          = $endAt
    end_detail        = $endDetail
    frames_presented  = Last-Int '(\d+) frames presented'
    presents_last_2s  = Last-Int 'activity \(2 s\): (\d+) presents'
    non_dark_percent  = $nonDark
    frame_size        = if ($dump) { "$($dump.width)x$($dump.height)" } else { $null }
    draws             = if ($draws.Success) { [long]$draws.Groups[1].Value } else { $null }
    draws_skipped     = if ($draws.Success) { [long]$draws.Groups[2].Value } else { $null }
    dispatches_skipped = if ($draws.Success) { [long]$draws.Groups[4].Value } else { $null }
    peak_commit_mb    = Max-Int 'memory: (\d+) MB committed'
    peak_video_mb     = Max-Int 'video memory (\d+) MB local'
    game_state        = $ctx['Game State']
    game_frame        = $ctx['Frame']
    game_running_time = $ctx['Game Running Time']
    game_scene        = $ctx['Current SRL Name']
    game_peds         = $ctx['Peds']
}

$null = New-Item -ItemType Directory -Force -Path $Out
$file = Join-Path $Out ("{0}-{1}.json" -f ($Label -replace '[^\w.-]', '_'), $started.ToString('yyyyMMdd-HHmmss'))
($result | ConvertTo-Json) | Set-Content -Path $file -Encoding utf8
$result | ConvertTo-Json
"saved: $file"
