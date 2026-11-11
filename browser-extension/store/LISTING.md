# Chrome 웹 스토어 제출 정보

Chrome 웹 스토어 개발자 대시보드(https://chrome.google.com/webstore/devconsole)에 그대로 입력할 내용이다.

## 등록 상태

- 게시자: newer.corpor@gmail.com
- 항목 ID: `hmgopkkefgfppfedloeponobiamikomf`
- 2026-10-26: 초안 작성 완료(패키지 0.1.0, 등록정보, 이미지, 개인정보 보호)
- 2026-10-26: 저장소 주소 변경 반영, 설명 문구 개편, 패키지 0.1.1
- 2026-10-26: 심사 제출 (공개, 전체 지역, 검토 통과 후 자동 게시). 제출 시 "광범위한 호스트 권한으로 검토가 길어질 수 있음" 안내가 나옴. 업로드 사이트를 미리 알 수 없어 http·https 전체가 필요하다고 판단해 그대로 제출
- 2026-10-29: 0.1.1 공개 확인. 업데이트 서버 302, 공개 패키지가 `browser-extension/extension`과 같음 (웹 스토어가 넣는 `update_url`만 다름)
- 2026-10-29: 0.2.0 준비 (선택 기능 `다운로드 파일명 정리`, 선택 권한 `downloads`). 아직 제출하지 않음. 제출 전에 개인정보처리방침 변경을 저장소에 푸시해야 한다
- 2026-11-11: 0.2.0 실제 화면 검증 완료 (Windows Chrome·Edge 157, 맥 Chrome 157). 다운로드 기능 스크린샷 추가

## 패키지

- 파일: `npm run package:extension` → `dist/browser-extension/hangulbond-extension-<version>.zip`
- 버전: `browser-extension/extension/manifest.json`의 `version`

## 스토어 등록정보

| 항목 | 값 |
|---|---|
| 이름 | manifest에서 자동 (`한글본드 - 업로드 파일명 자소 분리 방지`) |
| 요약 | manifest에서 자동 |
| 카테고리 | 도구 (Tools) |
| 언어 | 한국어 (영어 번역 포함) |
| 아이콘 | `browser-extension/extension/icons/icon-128.png` |
| 스크린샷 (1280×800) | `browser-extension/store/screenshot.png`, `browser-extension/store/screenshot-download.png` (0.2.0 추가) |
| 작은 프로모션 타일 (440×280) | `browser-extension/store/promo.png` |
| 홈페이지 | https://github.com/da-itda/mac-nfd-nfc-hangulbond |
| 지원 | https://github.com/da-itda/mac-nfd-nfc-hangulbond/issues |

### 상세 설명 (한국어)

```text
맥에서 보낸 파일, 받는 사람에게는 "ㅎㅏㄴㄱㅡㄹ.pdf"로 보이고 있지 않나요?

맥용 Chrome은 파일을 올릴 때 한글 파일명을 자음·모음이 분리된 형태(NFD)로 보냅니다. 내 Mac에서는 멀쩡해 보여도 Windows와 대부분의 웹 서비스에서는 이름이 깨져 보입니다.

한글본드는 파일이 올라가는 순간 이름을 완성형(NFC)으로 바꿔, 받는 사람도 "한글.pdf" 그대로 보게 합니다.

■ 이런 분께 필요합니다
· Mac에서 Chrome으로 메일에 파일을 첨부하는 분
· Google Drive, 게시판, 업무 시스템에 파일을 올리는 분
· Windows를 쓰는 동료·고객과 파일을 주고받는 분

■ 설치하면 끝
설정할 것이 없습니다. 파일 선택, 드래그 앤 드롭 업로드에서 자동으로 동작합니다.

■ 다운로드 파일명 정리 (선택)
일부 사이트는 내려받는 파일 이름을 자소 분리된 그대로 보냅니다. 이렇게 받은 파일은 Windows에서 이름이 깨져 보입니다. 확장 아이콘을 눌러 "다운로드 파일명 정리"를 켜면 저장하기 직전에 이름을 정상 형태로 바꿉니다. Windows에서도 동작합니다. 켤 때만 Chrome이 다운로드 관리 권한을 묻습니다.

■ 안전
· 파일 이름만 바꾸고 내용은 건드리지 않습니다.
· 이미 정상인 이름에는 개입하지 않습니다.
· 데이터를 수집·저장·전송하지 않습니다. 업로드 정리에는 별도 권한이 필요 없고, 다운로드 정리는 켤 때만 권한을 요청합니다.

Mac에 저장된 파일 이름까지 정리하려면 무료 한글본드 앱을 함께 사용하세요.
https://github.com/da-itda/mac-nfd-nfc-hangulbond
```

### 상세 설명 (English)

```text
Do your files sent from a Mac show up as "ㅎㅏㄴㄱㅡㄹ.pdf" on the other side?

Chrome on macOS uploads Korean file names in decomposed form (NFD). They look fine on your Mac but break on Windows and most websites.

HangulBond converts the name to NFC the moment a file is uploaded, so recipients see "한글.pdf".

Who needs it: Mac users who attach files in Chrome, upload to Google Drive or web forms, or share files with Windows users.
No setup. Works on file pickers and drag and drop.
Optional: turn on "Fix download file names" from the toolbar icon to save downloaded files whose names arrive decomposed in their normal form. Works on Windows too. Chrome asks for the downloads permission only when you turn it on.
Safe: only the file name changes, never the contents. No data is collected or sent. Upload fixing needs no extra permissions; download fixing asks for one only when you turn it on.
```

## 개인정보 보호 탭

| 항목 | 입력 |
|---|---|
| 단일 목적 | 파일을 올리거나 내려받을 때 한글 파일명을 유니코드 NFC 형태로 바꿔, 자소 분리되지 않게 한다. |
| 선택 권한 사유 (`downloads`, 0.2.0) | 사용자가 `다운로드 파일명 정리`를 켤 때만 요청한다. 내려받는 파일의 이름이 자소 분리(NFD)돼 있으면 `chrome.downloads.onDeterminingFilename`으로 NFC 이름을 제안한다. 다운로드 기록을 읽거나 저장·전송하지 않는다. |
| 호스트 권한 사유 (콘텐츠 스크립트 `http://*/*`, `https://*/*`) | 사용자가 파일을 올리는 사이트를 미리 알 수 없어서, 모든 http·https 페이지의 파일 선택·드롭 이벤트에서 파일명만 NFC로 바꾼다. 페이지 내용을 읽거나 저장·전송하지 않는다. |
| 원격 코드 사용 | 아니요 |
| 데이터 사용 | 수집하는 사용자 데이터 없음 (모든 항목 체크하지 않음) |
| 인증 | 판매·무관한 용도 사용·신용 평가 용도 사용을 하지 않음에 모두 동의 |
| 개인정보처리방침 URL | https://github.com/da-itda/mac-nfd-nfc-hangulbond/blob/main/docs/BROWSER_EXTENSION_PRIVACY.md (저장소에 푸시한 뒤 열리는지 확인) |
