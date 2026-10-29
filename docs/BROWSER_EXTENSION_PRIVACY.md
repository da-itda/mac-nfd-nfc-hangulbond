# 한글본드 브라우저 확장 개인정보처리방침

시행일: 2026-10-29

## 한국어

한글본드 브라우저 확장(이하 "확장")은 웹 페이지에 파일을 올릴 때, 그리고 사용자가 켠 경우 파일을 내려받을 때 파일명을 유니코드 NFC 형태로 바꾸는 기능만 수행합니다.

- **수집하는 정보:** 없습니다. 확장은 개인정보, 방문 기록, 파일 내용, 파일명을 수집하거나 저장하지 않습니다.
- **외부 전송:** 없습니다. 확장은 개발자 서버나 제3자에게 어떤 데이터도 보내지 않습니다. 파일은 사용자가 선택한 웹사이트로만, 원래 내용 그대로 전송됩니다.
- **처리 방식:** 파일명 변환은 사용자의 브라우저 안에서만 이루어집니다. 파일 내용은 읽거나 바꾸지 않습니다.
- **권한:** 업로드 보정을 위해 http·https 페이지에서만 동작하는 스크립트를 사용하며, 이를 위한 별도 권한은 요청하지 않습니다. 사용자가 `다운로드 파일명 정리`를 켤 때만 `다운로드 관리` 권한을 요청하며, 내려받는 파일의 이름을 정하는 순간에만 그 이름을 확인합니다. 다운로드 기록은 읽거나 보관하지 않으며, 기능을 끄면 권한을 반납합니다.
- **저장소:** 브라우저 저장소, 쿠키, 원격 코드를 사용하지 않습니다.

이 방침이 바뀌면 이 문서를 갱신하고 시행일을 고칩니다. 문의는 https://github.com/da-itda/mac-nfd-nfc-hangulbond/issues 로 해 주세요.

## English

The HangulBond browser extension only converts file names to Unicode NFC when you upload files on a web page and, if you turn it on, when you download files.

- **Data collected:** None. The extension does not collect or store personal data, browsing history, file contents, or file names.
- **Data shared:** None. The extension sends no data to the developer or any third party. Files are sent only to the website you chose, with their original contents.
- **Processing:** File name conversion happens entirely inside your browser. File contents are never read or modified.
- **Permissions:** It runs a content script on http and https pages to correct file uploads on any site, without requesting extra permissions for this. Only when you turn on "Fix download file names" does it request the "Manage your downloads" permission, and it checks a file's name only while the download is being named. It does not read or keep your download history, and it gives the permission back when you turn the feature off.
- **Storage:** The extension uses no browser storage, cookies, or remote code.

If this policy changes, this document and its effective date will be updated. Contact: https://github.com/da-itda/mac-nfd-nfc-hangulbond/issues
