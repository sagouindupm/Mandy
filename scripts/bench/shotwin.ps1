# shotwin.ps1 <out.png> : the game window's client area as a 640-wide PNG, and the mean brightness
# (0-255) of a strip at its left and right edges (a wall there is dark, open sky is bright).
param([string]$Out)
Add-Type -AssemblyName System.Drawing
Add-Type @'
using System; using System.Runtime.InteropServices;
public static class W {
    [StructLayout(LayoutKind.Sequential)] public struct RECT { public int L, T, R, B; }
    [StructLayout(LayoutKind.Sequential)] public struct POINT { public int X, Y; }
    [DllImport("user32.dll")] public static extern bool GetClientRect(IntPtr h, out RECT r);
    [DllImport("user32.dll")] public static extern bool ClientToScreen(IntPtr h, ref POINT p);
    [DllImport("user32.dll")] public static extern bool SetProcessDPIAware();
}
'@
[void][W]::SetProcessDPIAware()
$p = Get-Process victory -ErrorAction SilentlyContinue | Where-Object { $_.MainWindowTitle } | Select-Object -First 1
if (-not $p) { 'no window'; exit }
$r = New-Object W+RECT; [void][W]::GetClientRect($p.MainWindowHandle, [ref]$r)
$o = New-Object W+POINT; [void][W]::ClientToScreen($p.MainWindowHandle, [ref]$o)
$w = $r.R - $r.L; $h = $r.B - $r.T
if ($w -lt 100 -or $h -lt 100) { 'window too small'; exit }
$full = New-Object System.Drawing.Bitmap $w, $h
$g = [System.Drawing.Graphics]::FromImage($full)
$g.CopyFromScreen($o.X, $o.Y, 0, 0, (New-Object System.Drawing.Size $w, $h))
$g.Dispose()
$sw = 640; $sh = [int]($h * 640 / $w)
$small = New-Object System.Drawing.Bitmap $sw, $sh
$g = [System.Drawing.Graphics]::FromImage($small)
$g.DrawImage($full, 0, 0, $sw, $sh); $g.Dispose(); $full.Dispose()
function Strip($x0, $x1) {
    $s = 0; $n = 0
    for ($y = [int]($sh * 0.30); $y -lt [int]($sh * 0.70); $y += 3) {
        for ($x = $x0; $x -lt $x1; $x += 3) { $c = $small.GetPixel($x, $y); $s += ($c.R + $c.G + $c.B) / 3; $n++ }
    }
    [int]($s / $n)
}
$left = Strip 6 70; $right = Strip 570 634
$small.Save($Out, [System.Drawing.Imaging.ImageFormat]::Png); $small.Dispose()
"title='$($p.MainWindowTitle -replace '.*VIctory ','')' left=$left right=$right"
