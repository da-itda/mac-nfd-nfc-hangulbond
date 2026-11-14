; 한글본드 Windows 설치 파일 훅
; 삭제할 때 앱이 만든 등록을 함께 지운다 (macOS install.sh --uninstall과 같은 범위).
; 업데이트 중에는 설치 파일이 이전 버전 삭제를 /UPDATE로 실행하므로 지우지 않는다.
!macro NSIS_HOOK_POSTUNINSTALL
  ${If} $UpdateMode <> 1
    ; 로그인 시 자동 실행 (tauri-plugin-autostart가 만든 항목)
    DeleteRegValue HKCU "Software\Microsoft\Windows\CurrentVersion\Run" "HangulBond"
    ; Chrome 외부 확장 등록. Chrome에서 직접 설치한 확장은 Chrome에서 지운다.
    DeleteRegKey HKCU "Software\Google\Chrome\Extensions\hmgopkkefgfppfedloeponobiamikomf"
  ${EndIf}
!macroend

; 설치 파일을 직접 실행해 다시 설치하면 이전 버전 삭제가 /UPDATE 없이 돌아
; 위 훅이 자동 실행 항목을 지운다. 앱은 레지스트리를 기준으로 삼으므로 그대로 두면
; 다음 로그인부터 실행되지 않는다. 남아 있는 설정에서 자동 실행이 켜져 있었으면 되살린다.
!macro NSIS_HOOK_POSTINSTALL
  ${If} $UpdateMode <> 1
    Push $0
    Push $1
    Push $2
    SetShellVarContext current
    ClearErrors
    FileOpen $0 "$APPDATA\${BUNDLEID}\settings.json" r
    ${IfNot} ${Errors}
      ${Do}
        ClearErrors
        FileRead $0 $1
        ${If} ${Errors}
          ${Break}
        ${EndIf}
        ${StrLoc} $2 $1 '"launchAtLogin": true' ">"
        ${If} $2 != ""
          WriteRegStr HKCU "Software\Microsoft\Windows\CurrentVersion\Run" "HangulBond" "$INSTDIR\${MAINBINARYNAME}.exe"
          ${Break}
        ${EndIf}
      ${Loop}
      FileClose $0
    ${EndIf}
    Pop $2
    Pop $1
    Pop $0
  ${EndIf}
!macroend
