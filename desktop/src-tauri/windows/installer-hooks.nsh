; KiDo-Timer NSIS installer hooks (Tauri 2 `bundle.windows.nsis.installerHooks`).
;
; On-prem mode: the desktop app serves the Strafrichter UI to the judges'
; phones on TCP 8787-8797 (first free port). Windows Defender Firewall
; blocks inbound connections by default, so the installer adds an inbound
; rule for the app executable (private + public profiles) and the
; uninstaller removes it again. Manual fallback: see desktop/README.md.
;
; Runs elevated (installMode = perMachine). Failures are logged and never
; abort the (un)installation.

!define KIDO_FW_RULE "KiDo-Timer On-prem (Strafrichter)"

!macro NSIS_HOOK_POSTINSTALL
  DetailPrint "Firewall-Regel für den On-prem-Betrieb (TCP 8787-8797) wird eingerichtet ..."
  ; Remove a rule of an older installation first (idempotent re-install / update).
  nsExec::ExecToLog 'netsh advfirewall firewall delete rule name="${KIDO_FW_RULE}"'
  Pop $0
  nsExec::ExecToLog 'netsh advfirewall firewall add rule name="${KIDO_FW_RULE}" dir=in action=allow protocol=TCP localport=8787-8797 profile=private,public program="$INSTDIR\${MAINBINARYNAME}.exe" enable=yes'
  Pop $0
  StrCmp $0 "0" +2
    DetailPrint "Hinweis: Firewall-Regel konnte nicht angelegt werden (Code $0) - siehe README, manuelle Einrichtung."
!macroend

!macro NSIS_HOOK_PREUNINSTALL
  DetailPrint "Firewall-Regel für den On-prem-Betrieb wird entfernt ..."
  nsExec::ExecToLog 'netsh advfirewall firewall delete rule name="${KIDO_FW_RULE}"'
  Pop $0
!macroend
