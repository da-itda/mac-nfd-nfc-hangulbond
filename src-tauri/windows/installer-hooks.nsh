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
