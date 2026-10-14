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
  StrCpy $0 '"$SYSDIR\WindowsPowerShell\v1.0\powershell.exe" -NoProfile -ExecutionPolicy Bypass -File "$PLUGINSDIR\webview2.ps1"'
  StrCpy $1 ""
  Call ExecAndWait
  ${If} ${Errors}
  ${OrIf} $0 != 0
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

; $0 command line, $1 working directory (empty inherits the current directory).
; $0 is the process exit code afterwards. The error flag is set when the
; process cannot be started.
;
; NSIS ExecWait waits in WaitForSingleObject and only dispatches WM_PAINT.
; The launched host's first SetForegroundWindow posts to that foreground
; thread and never returns; killing this process is what lets the host
; continue. MsgWaitForMultipleObjects plus a full PeekMessage drain runs
; the same wait without leaving those messages queued.
Function ExecAndWait
  Push $2
  Push $3
  Push $4
  Push $5
  Push $6
  Push $7
  Push $8
  Push $9
  StrCpy $2 0
  StrCpy $3 0
  StrCpy $4 0
  StrCpy $5 0
  StrCpy $6 0
  StrCpy $7 0
  !if "${NSIS_PTR_SIZE}" == 8
    System::Alloc 104
  !else
    System::Alloc 68
  !endif
  Pop $2
  !if "${NSIS_PTR_SIZE}" == 8
    System::Alloc 24
  !else
    System::Alloc 16
  !endif
  Pop $3
  System::Alloc ${NSIS_PTR_SIZE}
  Pop $7
  System::Alloc 48
  Pop $4
  ${If} $2 = 0
  ${OrIf} $2 == ""
  ${OrIf} $2 == "error"
  ${OrIf} $3 = 0
  ${OrIf} $3 == ""
  ${OrIf} $3 == "error"
  ${OrIf} $4 = 0
  ${OrIf} $4 == ""
  ${OrIf} $4 == "error"
  ${OrIf} $7 = 0
  ${OrIf} $7 == ""
  ${OrIf} $7 == "error"
    ${If} $2 == "error"
    ${OrIf} $2 == ""
      StrCpy $2 0
    ${EndIf}
    ${If} $3 == "error"
    ${OrIf} $3 == ""
      StrCpy $3 0
    ${EndIf}
    ${If} $4 == "error"
    ${OrIf} $4 == ""
      StrCpy $4 0
    ${EndIf}
    ${If} $7 == "error"
    ${OrIf} $7 == ""
      StrCpy $7 0
    ${EndIf}
    Goto exec_and_wait_failed
  ${EndIf}
  !if "${NSIS_PTR_SIZE}" == 8
    System::Call 'ntdll::RtlZeroMemory(p r2, i 104)'
    System::Call 'ntdll::RtlZeroMemory(p r3, i 24)'
    System::Call '*$2(i 104)'
  !else
    System::Call 'ntdll::RtlZeroMemory(p r2, i 68)'
    System::Call 'ntdll::RtlZeroMemory(p r3, i 16)'
    System::Call '*$2(i 68)'
  !endif
  ${If} $1 == ""
    System::Call 'kernel32::CreateProcessW(p 0, w r0, p 0, p 0, i 0, i 0, p 0, p 0, p r2, p r3) i.r9'
  ${Else}
    System::Call 'kernel32::CreateProcessW(p 0, w r0, p 0, p 0, i 0, i 0, p 0, w r1, p r2, p r3) i.r9'
  ${EndIf}
  ${If} $9 = 0
    Goto exec_and_wait_failed
  ${EndIf}
  System::Call '*$3(p .r5, p .r6)'
  System::Call '*$7(p r5)'
exec_and_wait_loop:
  System::Call 'user32::MsgWaitForMultipleObjects(i 1, p r7, i 0, i 0xFFFFFFFF, i 0x04FF) i.r9'
  ${If} $9 = 1
    Goto exec_and_wait_pump
  ${EndIf}
  ${If} $9 != 0
    Goto exec_and_wait_failed
  ${EndIf}
  System::Call 'kernel32::GetExitCodeProcess(p r5, *i .r0) i.r9'
  ${If} $9 = 0
    Goto exec_and_wait_failed
  ${EndIf}
  ClearErrors
  Goto exec_and_wait_cleanup
exec_and_wait_pump:
  System::Call 'user32::PeekMessageW(p r4, p 0, i 0, i 0, i 1) i.r9'
  ${If} $9 = 0
    Goto exec_and_wait_loop
  ${EndIf}
  System::Call 'user32::TranslateMessage(p r4) i.r9'
  System::Call 'user32::DispatchMessageW(p r4) p.r9'
  Goto exec_and_wait_pump
exec_and_wait_failed:
  StrCpy $0 1
  SetErrors
exec_and_wait_cleanup:
  ${If} $6 != 0
    System::Call 'kernel32::CloseHandle(p r6)'
  ${EndIf}
  ${If} $5 != 0
    System::Call 'kernel32::CloseHandle(p r5)'
  ${EndIf}
  ${If} $2 != 0
    System::Free $2
  ${EndIf}
  ${If} $3 != 0
    System::Free $3
  ${EndIf}
  ${If} $4 != 0
    System::Free $4
  ${EndIf}
  ${If} $7 != 0
    System::Free $7
  ${EndIf}
  Pop $9
  Pop $8
  Pop $7
  Pop $6
  Pop $5
  Pop $4
  Pop $3
  Pop $2
FunctionEnd
