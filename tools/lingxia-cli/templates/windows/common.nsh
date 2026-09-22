!include "LogicLib.nsh"
!include "FileFunc.nsh"
!include "x64.nsh"
Unicode true
RequestExecutionLevel user
SetCompressor /SOLID lzma
ShowInstDetails show
ManifestDPIAware true
VIProductVersion "${FILE_VERSION}"
VIAddVersionKey /LANG=1033 "ProductName" "${PRODUCT}"
VIAddVersionKey /LANG=1033 "ProductVersion" "${VERSION}"
VIAddVersionKey /LANG=1033 "FileDescription" "${PRODUCT} distribution"
VIAddVersionKey /LANG=1033 "FileVersion" "${VERSION}"
VIAddVersionKey /LANG=1033 "LegalCopyright" ""

Function EnsureWebView2
  SetRegView 32
  ReadRegStr $0 HKLM "SOFTWARE\Microsoft\EdgeUpdate\Clients\{F3017226-FE2A-4295-8BDF-00C3A9A7E4C5}" "pv"
  ${If} $0 != ""
  ${AndIf} $0 != "0.0.0.0"
    Return
  ${EndIf}
  ReadRegStr $0 HKCU "SOFTWARE\Microsoft\EdgeUpdate\Clients\{F3017226-FE2A-4295-8BDF-00C3A9A7E4C5}" "pv"
  ${If} $0 != ""
  ${AndIf} $0 != "0.0.0.0"
    Return
  ${EndIf}
  MessageBox MB_YESNO|MB_ICONQUESTION "This application requires Microsoft Edge WebView2 Runtime. Download and install it now?" /SD IDYES IDYES install_webview
  SetErrorLevel 2
  Quit
install_webview:
  InitPluginsDir
  SetOutPath "$PLUGINSDIR"
  File /oname=webview2.ps1 "${WEBVIEW_SCRIPT}"
  ExecWait '"$SYSDIR\WindowsPowerShell\v1.0\powershell.exe" -NoProfile -ExecutionPolicy Bypass -File "$PLUGINSDIR\webview2.ps1"' $0
  ${If} $0 != 0
    MessageBox MB_OK|MB_ICONSTOP "WebView2 installation failed. Install Microsoft Edge WebView2 Runtime and try again." /SD IDOK
    SetErrorLevel 2
    Quit
  ${EndIf}
FunctionEnd

Function CheckArchitecture
!if "${ARCH}" == "x64"
  ${IfNot} ${RunningX64}
    MessageBox MB_OK|MB_ICONSTOP "This application requires 64-bit Windows." /SD IDOK
    SetErrorLevel 2
    Quit
  ${EndIf}
!endif
!if "${ARCH}" == "arm64"
  ${IfNot} ${IsNativeARM64}
    MessageBox MB_OK|MB_ICONSTOP "This application requires ARM64 Windows." /SD IDOK
    SetErrorLevel 2
    Quit
  ${EndIf}
!endif
FunctionEnd
