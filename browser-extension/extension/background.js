// 한글본드 브라우저 확장 — 다운로드 파일명 정리 (선택 기능)
// 서버가 자소 분리된 이름(NFD)을 그대로 보내면 Windows 탐색기 등에서 이름이 깨져 보인다.
// 사용자가 팝업에서 켜서 downloads 권한을 준 경우에만 저장 직전에 이름을 NFC로 바꾼다.
// 이미 NFC인 이름에는 개입하지 않으며 다운로드 기록을 읽거나 저장·전송하지 않는다.
const fixDownloadName = (item, suggest) => {
  const fixed = item.filename.normalize("NFC");
  if (fixed !== item.filename) {
    suggest({ filename: fixed, conflictAction: "uniquify" });
  }
};

// downloads 권한이 없으면 chrome.downloads가 없다. 권한을 받은 순간과 서비스 워커가 다시 뜰 때 연결한다.
const listen = () => {
  if (chrome.downloads && !chrome.downloads.onDeterminingFilename.hasListener(fixDownloadName)) {
    chrome.downloads.onDeterminingFilename.addListener(fixDownloadName);
  }
};

listen();
chrome.permissions.onAdded.addListener(listen);
chrome.permissions.onRemoved.addListener(() => {
  chrome.downloads?.onDeterminingFilename.removeListener(fixDownloadName);
});
