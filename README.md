<p align="center"><img src="app/hangulbond_img.png" width="96" alt="한글본드 아이콘"></p>

# 한글본드 — 맥 한글 파일명 자소 분리(NFD → NFC) 해결

맥에서 보낸 파일이 Windows나 웹에서 `ㅎㅏㄴㄱㅡㄹ.pdf`처럼 자음과 모음이 풀어져 보이나요?
한글본드는 맥의 한글 파일명과 폴더명을 정상 형태(NFC)로 정리하는 무료 macOS 앱과 Chrome 확장입니다.

[![최신 버전](https://img.shields.io/github/v/release/da-itda/mac-nfd-nfc-hangulbond?label=%EC%B5%9C%EC%8B%A0%20%EB%B2%84%EC%A0%84)](https://github.com/da-itda/mac-nfd-nfc-hangulbond/releases/latest)
![macOS Apple silicon](https://img.shields.io/badge/macOS-Apple%20silicon-black)

**[최신 버전 다운로드](https://github.com/da-itda/mac-nfd-nfc-hangulbond/releases/latest)** · [업데이트 소식](CHANGELOG.md) · [사용 설명서](docs/USER_GUIDE.md) · [문의·버그 신고](https://github.com/da-itda/mac-nfd-nfc-hangulbond/issues)

## 이런 문제를 해결합니다

- 맥에서 만든 파일을 Windows로 보내면 이름이 `ㅎㅏㄴㄱㅡㄹ`처럼 보인다.
- 맥 Chrome으로 메일, 드라이브, 게시판에 올린 파일 이름이 깨진다.
- USB, NAS, 클라우드 드라이브에서 한글 파일명이 자소 분리된다.

원인은 한글을 적는 방식의 차이입니다. 맥은 한글을 자음·모음을 나눈 형태(NFD)로 저장하거나 보내는 경우가 많고, Windows와 대부분의 웹은 완성형(NFC)을 씁니다.

## 구성

| 구성 | 하는 일 |
|---|---|
| 한글본드 앱 | 맥 디스크의 파일명과 폴더명을 NFC로 정리합니다. 지정한 폴더를 감시해 새 파일도 자동으로 정리합니다. |
| 한글본드 Chrome 확장 | 맥 Chrome이 업로드할 때 이름을 다시 분리해 보내는 문제를 막습니다. 선택 기능으로 내려받는 파일 이름도 정리합니다(Windows에서도 동작, 0.2.0 웹 스토어 심사 후 제공). |

## 설치

Apple silicon(M1 이후) Mac용입니다. Intel Mac은 지원하지 않습니다.

### 방법 1. 터미널로 한 번에 설치 (앱 + Chrome 확장)

`터미널` 앱을 열고 아래 한 줄을 붙여 넣은 뒤 Return 키를 누릅니다.

```bash
curl -fsSL https://raw.githubusercontent.com/da-itda/mac-nfd-nfc-hangulbond/main/install.sh | bash
```

- GitHub 최신 릴리스의 설치 파일을 내려받고, 개발자 서명과 Apple 공증을 확인한 뒤 `응용 프로그램` 폴더에 설치합니다.
- Chrome이 있으면 웹 스토어의 한글본드 확장을 함께 등록합니다. Chrome을 다시 열면 나오는 알림에서 `확장 프로그램 사용 설정`을 누르세요.
- macOS 보안 설정 때문에 자동 등록이 안 되면, 앱을 처음 실행할 때 Chrome 웹 스토어가 열립니다. `Chrome에 추가`를 누르세요.
- 앱만 설치하려면 끝에 옵션을 붙입니다: `... | bash -s -- --no-extension`
- 같은 명령을 다시 실행하면 최신 버전으로 바뀝니다.
- 실행 전에 스크립트 내용을 확인할 수 있습니다: [install.sh](install.sh)

### 방법 2. 직접 다운로드

1. [최신 릴리스](https://github.com/da-itda/mac-nfd-nfc-hangulbond/releases/latest)에서 `HangulBond_버전_aarch64.dmg`를 내려받습니다.
2. DMG를 열고 `한글본드.app`을 `응용 프로그램` 폴더로 끌어 놓습니다.
3. `응용 프로그램`에서 한글본드를 실행합니다. 메뉴 막대에 한글본드 아이콘이 나타납니다.
4. Chrome을 쓴다면 앱이 한글본드 확장을 Chrome에 등록합니다. Chrome을 다시 열면 나오는 알림에서 `확장 프로그램 사용 설정`을 누르세요. macOS 보안 설정 때문에 등록할 수 없으면 Chrome 웹 스토어가 열리니 `Chrome에 추가`를 누르세요. 나중에 추가하려면 앱 `정보` 탭의 `Chrome 확장` → `설치`를 누르거나 [Chrome 웹 스토어](https://chromewebstore.google.com/detail/hmgopkkefgfppfedloeponobiamikomf)에서 추가합니다.

## 사용 방법

1. 메뉴 막대의 한글본드 아이콘을 눌러 설정을 엽니다.
2. `폴더` 탭에서 `+`를 눌러 정리할 폴더를 추가합니다.
3. `검사`를 누르면 바뀔 이름 목록이 나옵니다.
4. 정리할 항목을 확인하고 `시작`을 누릅니다.
5. 새 파일도 자동으로 정리하려면 `일반` 탭에서 `자동 감시`를 켭니다.

Chrome 확장은 설치만 하면 됩니다. 파일을 올릴 때 자동으로 이름을 NFC로 바꿔 보냅니다.
내려받은 파일 이름이 깨진다면 Chrome 도구 모음의 한글본드 아이콘을 눌러 `다운로드 파일명 정리`를 켜세요. Chrome이 다운로드 관리 권한을 물으면 `허용`을 누릅니다. (확장 0.2.0부터, 웹 스토어 심사 후 제공)

자세한 내용은 [사용 설명서](docs/USER_GUIDE.md)와 [브라우저 확장 안내](docs/BROWSER_EXTENSION.md)를 참고하세요.

## 업데이트

- 앱이 켜질 때와 6시간마다 새 버전을 확인해 자동으로 설치합니다. 서명이 확인된 업데이트만 설치합니다. 설정 창을 보고 있거나 정리 중일 때는 기다렸다가 `정보` 탭에 알립니다.
- 0.1.6 이하 버전은 자동 확인 기능이 없으므로 `정보` 탭에서 `확인`을 한 번 눌러 주세요.
- Chrome 확장은 Chrome이 자동으로 업데이트합니다.
- 무엇이 바뀌었는지는 [업데이트 소식](CHANGELOG.md)과 [릴리스 목록](https://github.com/da-itda/mac-nfd-nfc-hangulbond/releases)에서 확인할 수 있습니다.
- 새 버전 알림을 받으려면 이 페이지 오른쪽 위의 `Watch` → `Custom` → `Releases`를 선택하세요.

## 삭제

터미널에서:

```bash
curl -fsSL https://raw.githubusercontent.com/da-itda/mac-nfd-nfc-hangulbond/main/install.sh | bash -s -- --uninstall
```

실행 중인 앱을 종료하고 앱, 로그인 시 자동 실행 항목, 설정·로그·캐시, 개인정보 보호 권한 기록, Chrome 확장 등록을 모두 지웁니다. 설정을 남기려면 끝에 `--keep-settings`를 붙입니다: `... | bash -s -- --uninstall --keep-settings`

직접 삭제하려면 메뉴 막대에서 `한글본드 종료`를 누른 뒤 `응용 프로그램`의 `한글본드.app`을 휴지통으로 옮깁니다. 설정과 로그는 `~/Library/Application Support/com.hangulbond.desktop`에 남으니 필요 없으면 지웁니다. Chrome 확장은 `chrome://extensions`에서 삭제합니다.

## 안전

- 파일 내용은 바꾸지 않고 이름만 바꿉니다.
- 정리하기 전에 바뀔 이름을 먼저 보여 주고, 원하는 항목만 고를 수 있습니다.
- 같은 이름이 이미 있으면 덮어쓰지 않습니다.
- 지정한 폴더만 다룹니다.
- Chrome 확장은 어떤 데이터도 수집하거나 보내지 않습니다. 업로드 정리는 별도 권한이 필요 없고, 다운로드 정리를 켤 때만 다운로드 관리 권한을 요청합니다. ([개인정보처리방침](docs/BROWSER_EXTENSION_PRIVACY.md))

## 자주 묻는 질문

**Safari에도 확장이 필요한가요?**
아니요. Safari는 업로드할 때 이름을 직접 NFC로 바꿔 보냅니다.

**앱으로 정리했는데 Chrome에서 올리면 또 깨집니다.**
맥 Chrome은 디스크의 이름이 정상이어도 업로드할 때 다시 분리합니다. Chrome 확장을 함께 설치하세요.

**메일 앱이나 텔레그램으로 보내면 깨집니다.**
이 앱들은 보낼 때 이름을 다시 분리해서, 맥 쪽에서는 고칠 수 없습니다. 카카오톡은 정상으로 보냅니다. 자세한 내용은 [문제 해결](docs/TROUBLESHOOTING.md)을 참고하세요.

**이름이 `???`나 알 수 없는 한자처럼 깨집니다.**
그것은 자소 분리가 아닌 문자 인코딩(CP949 등) 문제라 한글본드로 해결되지 않습니다.

## 문의

버그나 개선 의견은 [이슈](https://github.com/da-itda/mac-nfd-nfc-hangulbond/issues/new/choose)에 남겨 주세요.

## 개발

- 구현: Tauri 2, Rust, JavaScript
- 소스에서 실행: Node.js 20 이상과 Rust를 준비한 뒤 `npm install`, `npm run dev`
- 테스트: `npm test`, `npm run test:rust`, `npm run test:extension`
- Windows 빌드(개발 중): Windows에서 Node.js·Rust·Visual Studio Build Tools를 준비하고 `npm ci` 후 `npx tauri build --bundles nsis --config '{"bundle":{"createUpdaterArtifacts":false}}'`
- 문서 목록: [docs/README.md](docs/README.md)
