!include "${COMMON}"
!include "MUI2.nsh"
Name "${PRODUCT}"
OutFile "${OUTPUT}"
InstallDir "$LOCALAPPDATA\Programs\${APP_ID}"
InstallDirRegKey HKCU "Software\LingXia\Installations\${APP_ID}" "InstallLocation"
!define MUI_ABORTWARNING
!insertmacro MUI_PAGE_WELCOME
!insertmacro MUI_PAGE_INSTFILES
!insertmacro MUI_PAGE_FINISH
!insertmacro MUI_UNPAGE_CONFIRM
!insertmacro MUI_UNPAGE_INSTFILES
!insertmacro MUI_LANGUAGE "English"
!insertmacro MUI_LANGUAGE "SimpChinese"

Function .onInit
  SetShellVarContext current
  Call CheckArchitecture
FunctionEnd

Section "Install"
  Call EnsureWebView2
  SetRegView 32
  ; Never overwrite an unrelated directory supplied with /D.
  IfFileExists "$INSTDIR\app\*.*" owner_check
  IfFileExists "$INSTDIR\app.old\*.*" owner_check
  IfFileExists "$INSTDIR\app.new\*.*" owner_check
  IfFileExists "$INSTDIR\Uninstall.exe" owner_check
  IfFileExists "$INSTDIR\.lingxia-install-id" owner_check owner_ok
owner_check:
  ClearErrors
  FileOpen $0 "$INSTDIR\.lingxia-install-id" r
  IfErrors owner_bad
  FileRead $0 $1
  FileClose $0
  StrCmp $1 "${APP_ID}" owner_ok owner_bad
owner_bad:
  MessageBox MB_OK|MB_ICONSTOP "The installation directory belongs to another application." /SD IDOK
  SetErrorLevel 3
  Quit
owner_ok:
  ; A running host must exit before its payload can be replaced.
  IfFileExists "$INSTDIR\app\${EXE}" 0 app_closed
reset_running:
  StrCpy $2 0
check_running:
  ; Shared readers (e.g. scanners) are harmless; a running image denies writes.
  System::Call 'kernel32::CreateFileW(w "$INSTDIR\app\${EXE}", i 0x40000000, i 7, p 0, i 3, i 0, p 0) p.r0'
  ${If} $0 == -1
    ; Scanners may briefly retain a mapping after the host has exited.
    IntOp $2 $2 + 1
    ${If} $2 < 40
      Sleep 250
      Goto check_running
    ${EndIf}
    MessageBox MB_RETRYCANCEL|MB_ICONEXCLAMATION "Close ${PRODUCT} before installing this version." /SD IDCANCEL IDRETRY reset_running
    SetErrorLevel 4
    Quit
  ${EndIf}
  System::Call 'kernel32::CloseHandle(p r0)'
app_closed:
  RMDir /r "$INSTDIR\app.new"
  SetOutPath "$INSTDIR\app.new"
  ClearErrors
  File /r "${PAYLOAD}\*"
  IfErrors install_failed
  FileOpen $0 "$INSTDIR\app.new\.lingxia-distribution" w
  FileWrite $0 "nsis"
  FileClose $0
  SetOutPath "$INSTDIR"
  RMDir /r "$INSTDIR\app.old"
  IfFileExists "$INSTDIR\app\*.*" 0 swap_new
  ClearErrors
  Rename "$INSTDIR\app" "$INSTDIR\app.old"
  IfErrors install_failed
swap_new:
  ClearErrors
  Rename "$INSTDIR\app.new" "$INSTDIR\app"
  IfErrors rollback
  FileOpen $0 "$INSTDIR\.lingxia-install-id" w
  FileWrite $0 "${APP_ID}"
  FileClose $0
  WriteUninstaller "$INSTDIR\Uninstall.exe"
  WriteRegStr HKCU "Software\LingXia\Installations\${APP_ID}" "InstallLocation" "$INSTDIR"
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\${APP_ID}" "DisplayName" "${PRODUCT}"
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\${APP_ID}" "DisplayVersion" "${VERSION}"
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\${APP_ID}" "DisplayIcon" "$INSTDIR\app\${EXE}"
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\${APP_ID}" "InstallLocation" "$INSTDIR"
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\${APP_ID}" "UninstallString" '$\"$INSTDIR\Uninstall.exe$\"'
  WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\${APP_ID}" "QuietUninstallString" '$\"$INSTDIR\Uninstall.exe$\" /S'
  WriteRegDWORD HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\${APP_ID}" "NoModify" 1
  WriteRegDWORD HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\${APP_ID}" "NoRepair" 1
  SetOutPath "$INSTDIR\app"
  CreateShortcut "$SMPROGRAMS\${SHORTCUT}.lnk" "$INSTDIR\app\${EXE}"
  CreateShortcut "$DESKTOP\${SHORTCUT}.lnk" "$INSTDIR\app\${EXE}"
  RMDir /r "$INSTDIR\app.old"
  SetErrorLevel 0
  Goto done
rollback:
  Rename "$INSTDIR\app.old" "$INSTDIR\app"
install_failed:
  SetOutPath "$TEMP"
  RMDir /r "$INSTDIR\app.new"
  MessageBox MB_OK|MB_ICONSTOP "Installation failed. The previous application has been retained." /SD IDOK
  SetErrorLevel 5
  Quit
done:
SectionEnd

Section "Uninstall"
  SetShellVarContext current
  SetRegView 32
  FileOpen $0 "$INSTDIR\.lingxia-install-id" r
  IfErrors un_abort
  FileRead $0 $1
  FileClose $0
  StrCmp $1 "${APP_ID}" 0 un_abort
  IfFileExists "$INSTDIR\app\${EXE}" 0 un_remove
  StrCpy $2 0
un_check_running:
  System::Call 'kernel32::CreateFileW(w "$INSTDIR\app\${EXE}", i 0x40000000, i 7, p 0, i 3, i 0, p 0) p.r0'
  ${If} $0 == -1
    IntOp $2 $2 + 1
    ${If} $2 < 40
      Sleep 250
      Goto un_check_running
    ${EndIf}
    MessageBox MB_OK|MB_ICONSTOP "Close ${PRODUCT} before uninstalling." /SD IDOK
    Goto un_abort
  ${EndIf}
  System::Call 'kernel32::CloseHandle(p r0)'
un_remove:
  ; User data lives outside this owned payload and is deliberately retained.
  RMDir /r "$INSTDIR\app"
  RMDir /r "$INSTDIR\app.old"
  RMDir /r "$INSTDIR\app.new"
  IfFileExists "$INSTDIR\app\*.*" un_abort
  Delete "$SMPROGRAMS\${SHORTCUT}.lnk"
  Delete "$DESKTOP\${SHORTCUT}.lnk"
  DeleteRegValue HKCU "Software\Microsoft\Windows\CurrentVersion\Run" "${APP_ID}"
  DeleteRegKey HKCU "Software\LingXia\Installations\${APP_ID}"
  DeleteRegKey HKCU "Software\Microsoft\Windows\CurrentVersion\Uninstall\${APP_ID}"
  Delete "$INSTDIR\.lingxia-install-id"
  Delete "$INSTDIR\Uninstall.exe"
  RMDir "$INSTDIR"
  SetErrorLevel 0
  Goto un_done
un_abort:
  SetErrorLevel 4
un_done:
SectionEnd
