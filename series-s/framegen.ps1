# Sagouindupm | VIctory: the NVIDIA Streamline files frame generation needs (and DLSS super resolution with it),
# fetched from NVIDIA's own release on GitHub into the launchers' frame generation folder
# (%LOCALAPPDATA%\Sagouindupm\framegen by default). Run by both launchers ("framegen" in series-s\reglages.json says
# what to take, from where, and each DLL's SHA-256):
#
#   powershell -ExecutionPolicy Bypass -File series-s\framegen.ps1 [-Dest <folder>]
#
# NVIDIA's licence does not let us ship these files: each player gets them from NVIDIA, with NVIDIA's licences saved
# beside them (read them). Only the named files are taken out of the archive (about 70 MB of its 263 MB, by asking the
# server for their parts; the whole archive when it refuses). A file whose SHA-256 is not the one in reglages.json is
# refused and nothing is kept. dlssg-to-fsr3 (RTX 20/30, GTX 16) is not fetched here: it comes with the downloadable
# package (fg\) and the launchers put it in place before a session.
#
# The way of reading parts of the archive is after trustno1-coder/VIctory-NVIDIA scripts/get-nvidia-sdks.ps1
# (GPL-3.0-or-later), written again here.
param([string]$Dest = '')
$ErrorActionPreference = 'Stop'
$ProgressPreference = 'SilentlyContinue'   # Invoke-WebRequest's progress bar makes it many times slower
[Net.ServicePointManager]::SecurityProtocol = [Net.ServicePointManager]::SecurityProtocol -bor [Net.SecurityProtocolType]::Tls12

$Reglages = Get-Content (Join-Path $PSScriptRoot 'reglages.json') -Raw -Encoding UTF8 | ConvertFrom-Json
$Fg = $Reglages.framegen
$Sl = $Fg.streamline
if (-not $Dest) {
    $home_ = if ($env:SAGOUINDUPM_HOME) { $env:SAGOUINDUPM_HOME } else { Join-Path $env:LOCALAPPDATA 'Sagouindupm' }
    $Dest = Join-Path $home_ $Fg.dossier
}
$Dlls = @($Sl.fichiers)
$Licences = @($Sl.licences)

function Get-Sha256([string]$Path) { (Get-FileHash -Algorithm SHA256 -LiteralPath $Path).Hash.ToLower() }

# The SHA-256 reglages.json gives for a DLL ('' when none).
function Get-Expected([string]$Name) {
    $p = $Sl.sha256.PSObject.Properties | Where-Object { $_.Name -eq $Name } | Select-Object -First 1
    if ($p) { ([string]$p.Value).ToLower() } else { '' }
}

# The DLLs of $Dir that are missing or not NVIDIA's (SHA-256), as text; empty when all are right.
function Test-Files([string]$Dir) {
    $bad = @()
    foreach ($d in $Dlls) {
        $f = Join-Path $Dir $d
        $want = Get-Expected $d
        if (-not $want) { $bad += "$d (no SHA-256 in reglages.json)" }
        elseif (-not (Test-Path -LiteralPath $f)) { $bad += "$d (missing)" }
        elseif ((Get-Sha256 $f) -ne $want) { $bad += "$d (SHA-256 differs)" }
    }
    foreach ($l in $Licences) { if (-not (Test-Path -LiteralPath (Join-Path $Dir $l))) { $bad += "$l (missing)" } }
    $bad
}

# Bytes First..Last of a URL.
function Get-Range([string]$Url, [long]$First, [long]$Last) {
    $q = [Net.HttpWebRequest]::Create($Url)
    $q.UserAgent = 'victory-s-framegen'
    $q.AddRange($First, $Last)
    $r = $q.GetResponse()
    try {
        if ([int]$r.StatusCode -ne 206) { throw 'the server does not give parts of the file' }
        $m = New-Object IO.MemoryStream
        $r.GetResponseStream().CopyTo($m)
        , $m.ToArray()
    } finally { $r.Close() }
}

# The name a file of the archive is saved under in the folder, '' when it is not wanted: the DLLs and licences of
# bin/x64 (not bin/x64/development), and the SDK's own license.txt at the top.
function Get-Wanted([string]$Entry) {
    $e = $Entry -replace '\\', '/'
    if ($e -match '(^|/)bin/x64/([^/]+)$') {
        $n = $Matches[2]
        if (($Dlls -contains $n) -or ($n -match '\.license\.txt$')) { return $n }
        return ''
    }
    if ($e -match '^([^/]+/)?license\.txt$') { return 'license.txt' }
    ''
}

# The wanted files out of the zip on the server without downloading all of it: its directory is at its end, and
# each file in it can be asked for by its place. Returns how many were saved into $Into.
function Get-FromZipRanges([string]$Url, [string]$Into) {
    $h = [Net.HttpWebRequest]::Create($Url); $h.Method = 'HEAD'; $h.UserAgent = 'victory-s-framegen'
    $r = $h.GetResponse(); $real = $r.ResponseUri.AbsoluteUri; $length = $r.ContentLength; $r.Close()
    $tailLen = [Math]::Min($length, 2MB)
    $tail = Get-Range $real ($length - $tailLen) ($length - 1)
    $end = -1
    for ($i = $tail.Length - 22; $i -ge 0; $i--) {
        if ($tail[$i] -eq 0x50 -and $tail[$i + 1] -eq 0x4b -and $tail[$i + 2] -eq 5 -and $tail[$i + 3] -eq 6) { $end = $i; break }
    }
    if ($end -lt 0) { throw 'no zip directory found' }
    $dirSize = [BitConverter]::ToUInt32($tail, $end + 12)
    $dirAt = [BitConverter]::ToUInt32($tail, $end + 16)
    if ($dirAt -eq [uint32]::MaxValue) { throw 'a zip64 archive' }
    $dir = if ($dirAt -ge $length - $tailLen) {
        $from = [int]($dirAt - ($length - $tailLen)); , $tail[$from..($from + $dirSize - 1)]
    } else { Get-Range $real $dirAt ($dirAt + $dirSize - 1) }
    $got = 0
    $at = 0
    while ($at + 46 -le $dir.Length -and [BitConverter]::ToUInt32($dir, $at) -eq 0x02014b50) {
        $method = [BitConverter]::ToUInt16($dir, $at + 10)
        $packed = [BitConverter]::ToUInt32($dir, $at + 20)
        $size = [BitConverter]::ToUInt32($dir, $at + 24)
        $nameLen = [BitConverter]::ToUInt16($dir, $at + 28)
        $extraLen = [BitConverter]::ToUInt16($dir, $at + 30)
        $noteLen = [BitConverter]::ToUInt16($dir, $at + 32)
        $headerAt = [BitConverter]::ToUInt32($dir, $at + 42)
        $name = [Text.Encoding]::UTF8.GetString($dir, $at + 46, $nameLen)
        $at += 46 + $nameLen + $extraLen + $noteLen
        $to = Get-Wanted $name
        if (-not $to) { continue }
        # The file's own header (its extra field may be longer than the directory's), then its bytes.
        $piece = Get-Range $real $headerAt ($headerAt + 30 + $nameLen + 2048 + $packed - 1)
        if ([BitConverter]::ToUInt32($piece, 0) -ne 0x04034b50) { throw "no file header for $name" }
        $dataAt = 30 + [BitConverter]::ToUInt16($piece, 26) + [BitConverter]::ToUInt16($piece, 28)
        if ($dataAt + $packed -gt $piece.Length) { throw "the header of $name is longer than expected" }
        $out = [IO.MemoryStream]::new()
        if ($method -eq 0) { $out.Write($piece, $dataAt, [int]$packed) }
        elseif ($method -eq 8) {
            $in = [IO.MemoryStream]::new($piece, $dataAt, [int]$packed)
            $z = [IO.Compression.DeflateStream]::new($in, [IO.Compression.CompressionMode]::Decompress)
            $z.CopyTo($out); $z.Close()
        } else { throw "$name is packed in a way this script does not know ($method)" }
        if ($out.Length -ne $size) { throw "$name came out $($out.Length) bytes long, not $size" }
        [IO.File]::WriteAllBytes((Join-Path $Into $to), $out.ToArray())
        Write-Host ('  {0}  {1:N1} MB' -f $to, ($size / 1MB))
        $got++
    }
    $got
}

# The whole archive, then the wanted files out of it.
function Get-FromWholeZip([string]$Url, [string]$Into) {
    $zip = Join-Path $Into 'streamline-sdk.zip'
    Write-Host '  Downloading the whole archive instead (263 MB)...'
    if (Get-Command curl.exe -ErrorAction SilentlyContinue) {
        & curl.exe --location --fail --silent --show-error --output $zip $Url
        if ($LASTEXITCODE -ne 0) { throw "could not download $Url (curl $LASTEXITCODE)" }
    } else {
        Invoke-WebRequest -Uri $Url -OutFile $zip -UseBasicParsing
    }
    Add-Type -AssemblyName System.IO.Compression, System.IO.Compression.FileSystem
    $z = [IO.Compression.ZipFile]::OpenRead($zip)
    try {
        foreach ($e in $z.Entries) {
            $to = Get-Wanted $e.FullName
            if ($to) {
                [IO.Compression.ZipFileExtensions]::ExtractToFile($e, (Join-Path $Into $to), $true)
                Write-Host ('  {0}  {1:N1} MB' -f $to, ($e.Length / 1MB))
            }
        }
    } finally { $z.Dispose() }
    Remove-Item -LiteralPath $zip -Force
}

try {
    Write-Host "Frame generation files: NVIDIA Streamline $($Sl.version), from github.com/NVIDIA-RTX/Streamline (NVIDIA's own release)."
    Write-Host "Folder: $Dest"
    New-Item -ItemType Directory -Force $Dest | Out-Null
    $before = @(Test-Files $Dest)
    if (-not $before) {
        Write-Host 'All there already, SHA-256 checked.'
        exit 0
    }
    $part = Join-Path $Dest '.download'
    Remove-Item -Recurse -Force -LiteralPath $part -ErrorAction SilentlyContinue
    New-Item -ItemType Directory -Force $part | Out-Null
    try {
        try { [void](Get-FromZipRanges $Sl.url $part) }
        catch { Write-Host "  Taking the files out of the archive in place did not work ($($_.Exception.Message))." }
        if (@(Test-Files $part).Count -gt 0) { Get-FromWholeZip $Sl.url $part }
        $bad = @(Test-Files $part)
        if ($bad) { throw "Refused, nothing kept: $($bad -join ', ')." }
        Get-ChildItem -LiteralPath $part -File | Move-Item -Destination $Dest -Force
    } finally { Remove-Item -Recurse -Force -LiteralPath $part -ErrorAction SilentlyContinue }
    $bad = @(Test-Files $Dest)
    if ($bad) { throw "Still wrong after the download: $($bad -join ', ')." }
    Write-Host "Done, SHA-256 checked. NVIDIA's licences are beside the files: $(($Licences | ForEach-Object { Join-Path $Dest $_ }) -join ', ')."
    exit 0
} catch {
    Write-Host "Frame generation files: $($_.Exception.Message)"
    exit 1
}
