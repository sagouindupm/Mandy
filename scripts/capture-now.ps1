# Starts the frame capture of a run started with scripts\play-nvidia.ps1 -Capture (docs/replay.md):
# the recorder watches for this file. The game stands still for a few seconds while it copies the frame out.
$captures = Join-Path $env:LOCALAPPDATA 'victory\captures'
if (-not (Test-Path $captures)) { Write-Host "No ${captures}: start the game with play-nvidia.ps1 -Capture first."; return }
New-Item -ItemType File -Force (Join-Path $captures 'go') | Out-Null
Write-Host "capture asked for; 'frame recorder: the frame is captured' in the log means it is done"
