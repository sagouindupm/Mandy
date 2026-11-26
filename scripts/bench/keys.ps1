# keys.ps1 -At <seconds list> [-Vk -Scan] : hold a key (Enter unless said; Neptune: Space 0x20/0x39) 200 ms in the window at each time (seconds from now).
# Windows may refuse to bring the window forward (foreground lock): an Alt tap unlocks it, and each press checks the window is in front first (3 tries).
param([string]$Title = "", [string]$At, [int]$Vk = 0x0D, [int]$Scan = 0x1C)
Add-Type @'
using System; using System.Runtime.InteropServices;
public static class K {
    [DllImport("user32.dll")] public static extern void keybd_event(byte vk, byte scan, uint flags, UIntPtr extra);
    [DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr h);
    [DllImport("user32.dll")] public static extern IntPtr GetForegroundWindow();
    [DllImport("user32.dll")] public static extern bool ShowWindow(IntPtr h, int cmd);
    [DllImport("user32.dll")] public static extern void SwitchToThisWindow(IntPtr h, bool alt);
}
'@
$t0 = Get-Date
foreach ($s in ($At -split ',')) {
    $wait = [double]$s - ((Get-Date) - $t0).TotalSeconds
    if ($wait -gt 0) { Start-Sleep -Milliseconds ([int]($wait * 1000)) }
    # The game process itself (a browser tab titled after a VIctory repo matched the title before).
    $p = Get-Process -Name victory -ErrorAction SilentlyContinue | Where-Object { $_.MainWindowHandle -ne 0 } | Select-Object -First 1
    if (-not $p) { "$s s: no window"; continue }
    $h = $p.MainWindowHandle
    $front = $false
    for ($try = 0; $try -lt 3 -and -not $front; $try++) {
        if ($try -gt 0) {
            [K]::keybd_event(0x12, 0x38, 0, [UIntPtr]::Zero); [K]::keybd_event(0x12, 0x38, 2, [UIntPtr]::Zero)
        }
        [void][K]::ShowWindow($h, 9)
        [K]::SwitchToThisWindow($h, $true)
        [void][K]::SetForegroundWindow($h)
        Start-Sleep -Milliseconds 200
        $front = ([K]::GetForegroundWindow() -eq $h)
    }
    [K]::keybd_event([byte]$Vk, [byte]$Scan, 0, [UIntPtr]::Zero)
    Start-Sleep -Milliseconds 200
    [K]::keybd_event([byte]$Vk, [byte]$Scan, 2, [UIntPtr]::Zero)
    "$s s: key sent to '$($p.MainWindowTitle)' (in front: $front)"
}
