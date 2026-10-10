; Installer hooks, wired in through bundle.windows.nsis.installerHooks in
; tauri.conf.json. The template already includes LogicLib and System.

; After installing or updating, ask Windows to refresh its icons. Without
; this, shortcuts and taskbar pins keep showing an icon cached from an
; earlier version installed at the same path.
!macro NSIS_HOOK_POSTINSTALL
  ; SHCNE_ASSOCCHANGED with SHCNF_FLUSH: the shell drops its cached icons.
  System::Call "shell32::SHChangeNotify(i 0x08000000, i 0x1000, p 0, p 0)"
  ; Rebuild the per-user icon cache (Windows 10 and 11). Best effort: the
  ; 64-bit tool sits in Sysnative when seen from this 32-bit installer.
  ${If} ${FileExists} "$WINDIR\Sysnative\ie4uinit.exe"
    ExecWait '"$WINDIR\Sysnative\ie4uinit.exe" -show'
  ${ElseIf} ${FileExists} "$SYSDIR\ie4uinit.exe"
    ExecWait '"$SYSDIR\ie4uinit.exe" -show'
  ${EndIf}
  ClearErrors
!macroend
