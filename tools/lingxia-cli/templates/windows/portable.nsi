!include "${COMMON}"
Name "${PRODUCT}"
OutFile "${OUTPUT}"
SilentInstall silent
AutoCloseWindow true

Section
  Call CheckArchitecture
  Call EnsureWebView2
  InitPluginsDir
  SetOutPath "$PLUGINSDIR\app"
  ClearErrors
  File /r "${PAYLOAD}\*"
  IfErrors failed
  FileOpen $0 "$PLUGINSDIR\app\.lingxia-distribution" w
  FileWrite $0 "portable"
  FileClose $0
  System::Call 'kernel32::SetEnvironmentVariableW(w "LINGXIA_PORTABLE_EXECUTABLE", w "$EXEPATH")'
  System::Call 'kernel32::GetCurrentProcessId() i.r0'
  System::Call 'kernel32::SetEnvironmentVariableW(w "LINGXIA_PORTABLE_LAUNCHER_PID", w "$0")'
!if "${PORTABLE_DATA}" == "true"
  ClearErrors
  CreateDirectory "$EXEDIR\data\${APP_ID}"
  IfErrors failed
  System::Call 'kernel32::SetEnvironmentVariableW(w "LINGXIA_STATE_ROOT", w "$EXEDIR\data\${APP_ID}")'
!endif
  ${GetParameters} $2
  StrCpy $0 '"$PLUGINSDIR\app\${EXE}" $2'
  StrCpy $1 "$PLUGINSDIR\app"
  ClearErrors
  Call ExecAndWait
  IfErrors failed
  SetOutPath "$TEMP"
  RMDir /r "$PLUGINSDIR\app"
  SetErrorLevel $0
  Quit
failed:
  MessageBox MB_OK|MB_ICONSTOP "Unable to unpack or start ${PRODUCT}." /SD IDOK
  SetErrorLevel 1
SectionEnd
