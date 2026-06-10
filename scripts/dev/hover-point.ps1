# Dev-only helper: move the real cursor over a point inside a window
# (offsets from the window's top-right corner) so hover chrome states can be
# screenshotted with `lxdev app screenshot`.
param(
    [int]$Hwnd,
    [int]$FromRight = 23,
    [int]$FromTop = 16
)

Add-Type @"
using System;
using System.Runtime.InteropServices;
public static class Win32Hover {
    [StructLayout(LayoutKind.Sequential)]
    public struct RECT { public int Left, Top, Right, Bottom; }
    [DllImport("user32.dll")]
    public static extern bool GetWindowRect(IntPtr hWnd, out RECT rect);
    [DllImport("user32.dll")]
    public static extern bool SetCursorPos(int x, int y);
}
"@

$rect = New-Object Win32Hover+RECT
[Win32Hover]::GetWindowRect([IntPtr]$Hwnd, [ref]$rect) | Out-Null
$x = $rect.Right - $FromRight
$y = $rect.Top + $FromTop
[Win32Hover]::SetCursorPos($x, $y) | Out-Null
Start-Sleep -Milliseconds 250
"cursor at $x,$y (window $($rect.Left),$($rect.Top))-($($rect.Right),$($rect.Bottom))"
