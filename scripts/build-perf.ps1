# The play build for speed: target\perf\victory.exe (Cargo.toml [profile.perf]):
# no diagnostics feature, fat LTO, code generated for this machine's processor
# (-Cpu native, or a name such as znver3). Build with the game closed: the
# build uses every core. Play it with: scripts\play-nvidia.ps1 -Build perf ...
param([string]$Cpu = 'native')
$root = Join-Path $PSScriptRoot '..'
Push-Location $root
try {
    $env:RUSTFLAGS = "-C target-cpu=$Cpu"
    cargo build --locked --profile perf -p victory
} finally {
    Remove-Item Env:RUSTFLAGS -ErrorAction SilentlyContinue
    Pop-Location
}
