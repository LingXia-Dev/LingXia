# Dev-only helper: post a synthetic left-click (WM_LBUTTONDOWN/UP) at client
# coordinates of a window, for driving chrome elements from non-interactive
# sessions where SetCursorPos has no effect.
param(
    [int]$Hwnd,
    [int]$X,
    [int]$Y
)

Add-Type @"
using System;
using System.Runtime.InteropServices;
public static class Win32Click {
    [DllImport("user32.dll")]
    public static extern bool PostMessage(IntPtr hWnd, uint msg, IntPtr wParam, IntPtr lParam);
}
"@

$lp = [IntPtr](($Y -shl 16) -bor ($X -band 0xFFFF))
$h = [IntPtr]$Hwnd
[Win32Click]::PostMessage($h, 0x0201, [IntPtr]1, $lp) | Out-Null  # WM_LBUTTONDOWN, MK_LBUTTON
Start-Sleep -Milliseconds 60
[Win32Click]::PostMessage($h, 0x0202, [IntPtr]0, $lp) | Out-Null  # WM_LBUTTONUP
"clicked client $X,$Y on $Hwnd"
