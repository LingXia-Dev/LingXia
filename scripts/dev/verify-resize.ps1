# Dev-only helper: programmatically resize the main LingXia window through a
# sweep of sizes (exercising the live WM_WINDOWPOSCHANGED layout path), then
# leave it at the final size. Used with `lxdev app screenshot` to verify the
# webview and chrome adapt without ghost rectangles.
param(
    [int]$Hwnd,
    [int]$FinalW = 1400,
    [int]$FinalH = 900
)

Add-Type @"
using System;
using System.Runtime.InteropServices;
public static class Win32Resize {
    [DllImport("user32.dll")]
    public static extern bool SetWindowPos(IntPtr hWnd, IntPtr after, int x, int y, int cx, int cy, uint flags);
}
"@

$SWP_NOMOVE = 0x0002
$SWP_NOZORDER = 0x0004
$h = [IntPtr]$Hwnd

# Sweep: many small steps approximate an interactive drag.
for ($w = 900; $w -le $FinalW; $w += 50) {
    $hgt = [int](700 + ($w - 900) * ($FinalH - 700) / ($FinalW - 900))
    [Win32Resize]::SetWindowPos($h, [IntPtr]::Zero, 0, 0, $w, $hgt, $SWP_NOMOVE -bor $SWP_NOZORDER) | Out-Null
    Start-Sleep -Milliseconds 30
}
"resized to ${FinalW}x${FinalH}"
