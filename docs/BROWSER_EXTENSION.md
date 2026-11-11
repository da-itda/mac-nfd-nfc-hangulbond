# 한글본드 브라우저 확장

맥에서 Chrome으로 파일을 올리면, 디스크의 파일명이 정상이어도 업로드되는 이름이 `ㅎㅏㄴㄱㅡㄹ.pdf`처럼 자소 분리됩니다. 한글본드 확장은 웹 페이지가 파일을 읽기 직전에 파일명을 NFC로 바꿔 이 문제를 막습니다.

- 원인: 맥용 Chromium이 경로를 받을 때 HFS 분해형(NFD)으로 바꿔 업로드한다. 앱으로 디스크 이름을 고쳐도 업로드 단계에서 다시 분리된다 (`docs/TROUBLESHOOTING.md`).
- 결과: 확장을 켜면 받는 쪽(웹, Windows)에서 `한글.pdf`로 정상 표시된다.
- 0.2.0부터 다운로드 파일명 정리를 선택 기능으로 제공한다. 서버가 자소 분리된 이름을 보내면 저장하기 직전에 NFC로 바꾼다 (아래 `다운로드 파일명 정리`).

## 동작 범위

| 업로드 방식 | 결과 | 확인 |
|---|---|---|
| 파일 선택 (JavaScript 업로드) | NFC로 전송 | 자동 테스트 |
| 일반 HTML 폼 제출 | NFC로 전송 | 자동 테스트 |
| 드래그 앤 드롭 (함께 담긴 텍스트 유지) | NFC로 전송 | 자동 테스트 |
| Shadow DOM 안의 파일 입력칸 | NFC로 전송 | 자동 테스트 |
| 이미 NFC인 파일 | 개입하지 않음 | 자동 테스트 |
| 폴더째 올리기 | 변환하지 않음 (폴더 구조 유지) | 자동 테스트 |
| 붙여넣기 | 코드상 지원, 재현 환경을 만들지 못해 미확인 | - |
| 실제 Chrome 파일 선택창 (JS 업로드·일반 폼) | NFC로 전송 | 2026-10-26 설치한 확장으로 직접 확인 |
| Gmail 파일 첨부 | NFC로 첨부 | 2026-10-26 Aside에 확장 코드 주입해 확인 |
| Google Drive 파일 업로드 | NFC로 업로드 | 2026-10-26 Aside에 확장 코드 주입해 확인 |

- 파일 내용은 그대로 두고 이름만 바꾼다.
- 업로드 정리는 권한을 요청하지 않는다. http·https 페이지에서 콘텐츠 스크립트만 실행한다.
- 데이터를 수집·저장·전송하지 않는다 (`docs/BROWSER_EXTENSION_PRIVACY.md`).

알려진 제한:
- 드롭 처리 시 원래 이벤트를 멈추고 NFC 파일을 담은 이벤트를 다시 보낸다. 페이지가 처리하지 않고 브라우저 기본 동작에 맡기던 드롭(예: 일반 편집 영역에 이미지 삽입)은 일어나지 않을 수 있다.
- File System Access API(`showOpenFilePicker`)로 파일을 여는 사이트, Slack 같은 Electron 앱, Mail·텔레그램에는 적용되지 않는다.

## 다운로드 파일명 정리 (선택 기능, 0.2.0)

Chrome·Edge는 서버가 `filename*=UTF-8''…`로 보낸 이름은 NFC로 바꿔 저장하지만, 옛날 방식(`filename="…"`)이나 주소 경로의 이름은 NFD 그대로 저장한다. Windows 탐색기에서는 이 이름이 깨져 보인다 (`docs/TROUBLESHOOTING.md`).

- 켜는 방법: 확장 아이콘(또는 확장 `옵션`)을 눌러 `다운로드 파일명 정리`를 켠다. Chrome이 `다운로드 관리` 권한을 묻고, `허용`을 누르면 바로 동작한다. Windows 권한 창의 기본 버튼은 `거부`이고, Edge는 창이 뜬 뒤 잠시 `허용`을 누를 수 없다.
- 권한 창이 뜨면 확장 아이콘의 작은 창은 닫힌다. 허용하면 `background.js`가 권한 추가 이벤트로 바로 연결하므로 다시 열 필요는 없다. 다시 열면 스위치가 켜져 있다.
- 끄는 방법: 같은 스위치를 끈다. 권한도 함께 반납한다.
- 설정을 따로 저장하지 않는다. `downloads` 권한이 있으면 켜진 것으로 본다.
- 기본은 꺼져 있다. `downloads`를 `optional_permissions`로 두어, 0.1.x 사용자에게 업데이트 시 새 권한 경고로 확장이 꺼지는 일이 없다.
- 동작: `background.js`가 `chrome.downloads.onDeterminingFilename`에서 이름이 NFC가 아닐 때만 NFC 이름을 제안한다. 같은 이름이 있으면 Chrome 기본 동작대로 `(1)`을 붙인다.

| 시나리오 | 결과 | 확인 |
|---|---|---|
| 기능 꺼짐(기본) | 받은 이름을 건드리지 않음 | 자동 테스트 |
| 옛날 방식 헤더의 NFD 이름 | NFC로 저장 | 자동 테스트 |
| 주소 경로의 NFD 이름 | NFC로 저장 | 자동 테스트 |
| 이미 NFC인 이름 | 그대로 | 자동 테스트 |
| 같은 이름이 이미 있음 | 덮어쓰지 않고 `(1)` 추가 | 자동 테스트 |
| 서비스 워커가 꺼진 상태 | 다운로드 때 다시 떠서 NFC로 저장 | 자동 테스트 |
| 실제 화면: 설정 탭에서 켜기 → 권한 창 허용 → 다운로드 → 끄기 → 다운로드 | NFC 저장, 끈 뒤에는 NFD 그대로 | 2026-10-29 Windows 11 Chrome 156, 2026-11-11 Edge 157·맥 Chrome 157 |
| 실제 화면: 도구 모음 아이콘으로 켜기 → 허용 → 다운로드 → 다시 열기 | NFC 저장, 다시 연 창에서 켜짐 | 2026-11-11 Windows 11 Chrome 157 (Edge 157은 다시 열기 제외) |
| `다운로드 전에 저장 위치 확인` 켬 | 저장 창에 처음부터 NFC 이름 표시, NFC로 저장 | 2026-11-11 Windows 11 Chrome 157 |

알려진 제한:
- zip 등 압축 파일 안의 이름은 바꾸지 못한다.
- 다른 확장도 `onDeterminingFilename`으로 이름을 바꾸면 Chrome이 먼저 등록된 확장의 제안을 따를 수 있다.

## 설치

- 터미널 설치: `install.sh`가 앱을 설치하면서 Chrome 외부 확장 등록 파일(`~/Library/Application Support/Google/Chrome/External Extensions/hmgopkkefgfppfedloeponobiamikomf.json`)을 만든다. Chrome을 다시 열면 사용 설정 알림이 나온다. 웹 스토어에 공개된 뒤에만 동작하며, 공개 여부는 Chrome 업데이트 서버 응답(302)으로 확인한다.
- 웹 스토어 등록 전: Chrome에서 `chrome://extensions` → 개발자 모드 → `압축해제된 확장 프로그램을 로드합니다` → `browser-extension/extension` 폴더 선택
- Chrome 웹 스토어: https://chromewebstore.google.com/detail/hmgopkkefgfppfedloeponobiamikomf (2026-10-29 공개)

## 개발

| 명령 | 내용 |
|---|---|
| `npm run test:extension` | 설치된 Chrome을 임시 프로필로 띄워 확장을 설치하고 업로드 7개, 다운로드 6개 시나리오를 검사한다. 평소 Chrome 프로필은 건드리지 않는다. |
| `npm run package:extension` | 웹 스토어 제출용 zip을 `dist/browser-extension/`에 만든다. |

- Windows에서는 `CHROME_PATH`에 `chrome.exe`나 `msedge.exe` 경로를 지정해 같은 테스트를 돌린다. 2026-10-29 Windows 11 Chrome 156·Edge 156 모두 13개 통과.
- 권한 요청 창은 자동으로 누를 수 없어서, 다운로드 기능을 켠 상태는 `downloads`를 필수 권한으로 바꾼 임시 사본으로 검사한다.
- 정식 Chrome은 `--load-extension`을 막는다. 테스트는 `--enable-unsafe-extension-debugging` + `--remote-debugging-pipe`로 실행하고 `Extensions.loadUnpacked`로 설치한다.
- 이 Mac(macOS 27)에서는 Chrome for Testing 156이 페이지를 열지 못해 사용하지 않았다.

## 웹 스토어 제출

1. `browser-extension/extension/manifest.json`의 `version`을 올린다.
2. `npm run test:extension`이 전체 통과하는지 확인한다.
3. `npm run package:extension`으로 zip을 만든다.
4. 개인정보처리방침이 공개 주소에서 열리는지 확인한다 (저장소 푸시 필요).
5. 개발자 대시보드에서 zip을 올리고 `browser-extension/store/LISTING.md` 내용을 입력한 뒤 심사를 제출한다.
