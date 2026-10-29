// 다운로드 파일명 정리는 downloads 권한이 있을 때 켜진 것으로 본다. 설정을 따로 저장하지 않는다.
const DOWNLOADS = { permissions: ["downloads"] };
const toggle = document.getElementById("downloads");

for (const element of document.querySelectorAll("[data-i18n]")) {
  element.textContent = chrome.i18n.getMessage(element.dataset.i18n);
}

const refresh = async () => {
  toggle.checked = await chrome.permissions.contains(DOWNLOADS);
};

// 권한 요청 창이 뜨면 팝업이 닫힐 수 있다. 그래도 권한을 받으면 background.js가 바로 연결한다.
toggle.addEventListener("change", async () => {
  if (toggle.checked) {
    await chrome.permissions.request(DOWNLOADS);
  } else {
    await chrome.permissions.remove(DOWNLOADS);
  }
  refresh();
});

refresh();
