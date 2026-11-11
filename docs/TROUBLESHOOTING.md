# 문제 해결

## 디스크는 NFC인데 보내면 자소가 깨진다 (메일·텔레그램·웹 업로드)

- 원인: 한글본드는 디스크의 파일명만 NFC로 바꾼다. 보내는 앱이 파일명을 다시 읽을 때 macOS API(`URL(fileURLWithPath:)`, 클립보드 file-url 등)가 NFD로 돌려주고, 앱이 이를 NFC로 바꾸지 않으면 NFD로 전송된다.
- 결과: Windows나 웹 서버에서 `ㅎㅏㄴㄱㅡㄹ`처럼 보인다. 맥에서 맥으로 보낼 때는 두 형태 모두 정상으로 보여서 드러나지 않는다.

전송 경로별 확인 결과:

| 경로 | 보내는 파일명 | 확인 방법 |
|---|---|---|
| 카카오톡 (26.8.0) | NFC (앱이 자체 변환) | 실행 파일에 NFC 변환 API 포함, 사용자 테스트 |
| Mail | NFD | 사용자 테스트 |
| 텔레그램 macOS (12.10) | NFD | 사용자 테스트, 실행 파일에 NFC 변환 API 없음 |
| Chrome 156 파일 선택 업로드 | NFD (디스크가 NFC여도) | 2026-10-26 로컬 업로드 서버로 재현 |
| Aside 1.0.1022 (Chromium 계열) 파일 선택 업로드 | NFD | 같은 방법으로 재현 |
| Safari 27 파일 선택 업로드 | NFC (디스크가 NFD여도 변환) | 같은 방법으로 재현 |

Chromium(Chrome·Aside·Edge·Electron 앱)이 맥에서만 NFD로 보내는 이유:
- Chromium의 macOS 전용 코드가 경로를 받을 때 HFS식 분해형으로 바꾼다 (`base/apple/foundation_util.mm` `CFStringToFilePath` → `FilePath::GetHFSDecomposedForm`, `IS_APPLE` 전용).
- 경로를 직접 넘겨 이 변환을 건너뛰면 Chrome은 NFC를 그대로 보낸다 (2026-10-26 CDP 실험). 즉 디스크 이름과 상관없이 맥 Chromium에서는 NFD가 된다.
- Windows판에는 이 변환이 없다 (`ui/shell_dialogs/execute_select_file_win.cc`). Windows 디스크 이름이 NFC면 NFC로 전송될 것으로 보인다. 실제 Windows 테스트는 아직 하지 않았다.
- 페이지가 파일을 읽기 전에 파일명을 NFC로 바꾸는 확장으로 파일 선택·일반 폼·드래그 앤 드롭 업로드를 NFC로 보낼 수 있다 (`npm run test:extension`). Electron 앱에는 적용할 수 없다.

해결 방향:
- 보내는 쪽: 맥 Chrome 웹 업로드는 한글본드 브라우저 확장으로 보정한다 (`docs/BROWSER_EXTENSION.md`). Safari는 확장 없이 정상이다. Mail·텔레그램·Electron 앱은 고칠 수 없다.
- 받는 쪽: 직접 운영하는 웹 서비스라면 서버에서 업로드 파일명을 NFC로 정규화한다(PHP `Normalizer::normalize($name, Normalizer::FORM_C)`, JS `name.normalize("NFC")`, Python `unicodedata.normalize("NFC", name)`).
- Windows 수신자: Chrome·Edge로 받는다면 확장 0.2.0의 `다운로드 파일명 정리`를 켠다. 이미 받은 파일은 Windows용 한글본드로 정리한다 (계획 검토 중).

## Windows에서 받은 파일 이름이 깨진다 (다운로드)

2026-10-29 Windows 11 26H2, Chrome 156·Edge 156에서 로컬 서버가 NFD 이름을 보내고 브라우저가 실제로 저장한 이름을 확인했다. 맥 Chrome 156도 같은 결과다.

| 서버가 이름을 보내는 방식 | 저장된 이름 |
|---|---|
| `filename*=UTF-8''…` (표준) | NFC로 바뀜 |
| `filename="…"`와 `filename*`를 함께 | NFC로 바뀜 |
| `filename="…"`만 (UTF-8 그대로) | NFD 유지 |
| 주소 경로의 파일 이름 | NFD 유지 |

- Windows 탐색기는 NFD 이름을 자음·모음이 흩어진 형태로 보여 준다 (탐색기 캡처로 확인).
- 즉 받는 쪽 문제는 서버가 이름을 어떤 방식으로 보내는지에 달려 있다. 실제 서비스(Gmail, Drive, 게시판)의 방식은 아직 확인하지 않았다.
- 해결: Chrome 확장 0.2.0의 `다운로드 파일명 정리`를 켜면 위 두 경우도 NFC로 저장한다 (`docs/BROWSER_EXTENSION.md`). 이미 받은 파일이나 압축 파일 안의 이름은 바꾸지 못한다.
- 시험 스크립트: Windows `C:\Users\R\Documents\nfc_nfd\download-test\dl-test.mjs`

## Windows: 맥 Finder로 압축한 zip이 풀리지 않거나 이름이 깨진다

- 원인: 맥 Finder(`ditto`)는 zip 안 이름을 UTF-8로 저장하지만 UTF-8 표시를 하지 않는다. Windows 기본 도구(탐색기, `tar`, `Expand-Archive`)는 이 이름을 CP949로 읽어 문자가 깨지거나 "잘못된 문자"로 풀지 못한다 (2026-11-11 Windows 11 26H2 확인).
- 결과: 자소 분리와는 다른 문제라 한글본드로 고칠 수 없다.
- 해결: 반디집·7-Zip처럼 UTF-8을 인식하는 프로그램으로 푼다. 이렇게 풀면 이름이 자소 분리(NFD) 상태로 나오고, 한글본드가 감시 폴더에서 바로 정리한다. UTF-8 표시가 있는 zip은 탐색기로 풀어도 같다.

## Windows: 파일 이름이 바로 바뀌지 않는다 (사용 중인 파일)

- 원인: Windows는 다른 프로그램이 열고 있는 파일의 이름을 바꿀 수 없다 (os error 32).
- 결과: 로그에 "다른 프로그램이 파일을 사용 중입니다. 닫으면 다시 시도합니다."가 한 번 남는다.
- 해결: 파일을 닫으면 주기 검사에서 정리된다. 주기는 보통 5초이고, 감시 폴더가 아주 크면 검사에 걸린 시간의 50배(최대 1분)로 늘어난다.

## Windows: `cargo test`가 0xc0000139(STATUS_ENTRYPOINT_NOT_FOUND)로 시작하지 못한다

- 원인: tauri-build는 공용 컨트롤 v6 manifest를 앱 실행 파일에만 넣는다. 테스트 실행 파일은 예전 공용 컨트롤을 불러 진입점을 찾지 못한다.
- 해결: `src-tauri/build.rs`가 `windows-app-manifest.xml`을 링커 옵션으로 모든 실행 파일에 넣는다.

## Windows: 트레이 아이콘이 보이지 않는다

- Windows 11은 새 트레이 아이콘을 작업 표시줄 오른쪽 `^`(숨겨진 아이콘) 안에 둔다. 끌어서 작업 표시줄로 옮기거나, 설정 > 개인 설정 > 작업 표시줄 > 기타 시스템 트레이 아이콘에서 켠다.
- 아이콘 색은 작업 표시줄 테마에 맞춘다(밝게: 검은색, 어둡게: 흰색).

## 삭제 명령 후에도 메뉴 막대에 한글본드가 남는다

- 원인: macOS가 실행 중인 앱 경로를 자소 분리형(NFD)으로 기록한다. 이전 `install.sh`는 완성형 경로 `/Applications/한글본드.app/...`로 프로세스를 찾아 종료하지 못하고 앱 파일만 지웠다. 앱 파일이 지워진 뒤에는 AppleScript 종료 명령도 앱을 찾지 못한다.
- 해결: 실행 파일 이름(`/Contents/MacOS/hangul-nfc-normalizer`)으로 찾아 종료 요청 → TERM → KILL 순서로 끈다. 이전 버전 사본도 이름 대신 번들 ID로 확인해 지운다. 경로 비교에 한글 이름을 쓰지 않는다.

## 터미널 재설치 때 관리자 암호를 묻는다

- 원인: 한 번 실행된 앱은 macOS 앱 보호(`com.apple.macl`) 때문에 쓰기 권한 검사(`[ -w ]`, `access()`)가 거짓으로 나온다. 실제 이동·삭제는 일반 권한으로 된다.
- 해결: `install.sh`는 미리 검사하지 않고 일반 권한으로 먼저 실행한 뒤 실패할 때만 `sudo`를 쓴다. 기존 앱은 옆으로 옮겨 두었다가 새 앱 복사가 끝나면 지운다.

## 앱이나 터미널 설치가 Chrome 확장을 등록하지 못한다

- 원인: macOS 27(2026-10-29 확인, 27.0.1)은 다른 앱의 데이터 폴더(`~/Library/Application Support/Google/Chrome`)를 읽거나 쓰는 것을 막는다. Finder·`open`으로 실행한 앱과 기본 `터미널`은 `Operation not permitted`가 나고 알림창도 뜨지 않는다. `전체 디스크 접근` 권한이 있는 터미널(예: Ghostty)이나 터미널에서 실행 파일을 직접 띄우면 된다. 그래서 개발 실행(`npm run dev`)에서는 재현되지 않는다.
- 확인: 작은 AppleScript 앱(`osacompile`)을 `open`으로 실행해 `ls`·`mkdir`가 막히는지 본다. 네트워크(웹 스토어 302)는 정상이다.
- 해결: 등록이 막히면 0.1.8부터 처음 설치한 앱이 Chrome 웹 스토어를 한 번 열고, `정보` 탭에 `웹 스토어에서 설치`를 보여 준다. `install.sh`는 멈추지 않고 웹 스토어 주소를 안내한다. 이 환경에서는 확장이 이미 설치됐는지도 알 수 없다.

## 프로젝트를 옮긴 뒤 `cargo test`·`npm run dev`가 실패한다

- 원인: `src-tauri/target/`에 이전 경로(SynologyDrive)가 남아 있다.
- 결과: `failed to read plugin permissions ... No such file or directory`.
- 해결: `cd src-tauri && cargo clean` 후 다시 빌드한다.
