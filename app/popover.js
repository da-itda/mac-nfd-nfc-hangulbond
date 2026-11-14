// 알림 영역 팝오버(맥 메뉴 막대 클릭, Windows 작업 표시줄 오른쪽 클릭):
// 감시 상태, 최근 정리 3개, 감시 폴더, 맨 아래 업데이트·설정·종료 단추.
// 열릴 때마다(tray_popover.rs가 popover-shown을 보냄) 새로 읽고, 내용 높이에 맞춰 창 높이를 바꾼다.
const { invoke } = window.__TAURI__.core;
const { listen } = window.__TAURI__.event;
const appWindow = window.__TAURI__.window.getCurrentWindow();

const isWindows = navigator.userAgent.includes("Windows");
document.documentElement.classList.add(isWindows ? "platform-windows" : "platform-mac");

const RECENT_LIMIT = 3;
const FOLDER_LIMIT = 3;
// 팝오버를 열 때 마지막 확인에서 이만큼 지났으면 새 버전을 다시 확인한다.
const UPDATE_RECHECK_MS = 30 * 60 * 1000;

const panel = document.querySelector(".popover");
const statusDot = document.querySelector("#status-dot");
const statusText = document.querySelector("#status-text");
const recentList = document.querySelector("#recent-list");
const folderList = document.querySelector("#folder-list");
const appVersion = document.querySelector("#app-version");
const updateButton = document.querySelector("#update-button");
const updateIconBox = document.querySelector("#update-icon-box");
const updateIcon = document.querySelector("#update-icon");
const updateText = document.querySelector("#update-text");

// idle(확인 전) · checking · latest · available · downloading · installing · busy(다른 곳에서 진행 중) · failed
let updateState = "idle";
let newVersion = "";
let lastUpdateCheck = 0;

for (const button of document.querySelectorAll("[data-action]")) {
  button.addEventListener("click", () => {
    invoke("popover_action", { action: button.dataset.action }).catch((error) => {
      console.warn("popover action failed", error);
    });
  });
}

document.addEventListener("keydown", (event) => {
  if (event.key === "Escape") {
    appWindow.hide();
  }
});

updateButton.addEventListener("click", () => {
  if (updateState === "available") {
    installUpdate();
  } else if (updateState !== "checking" && updateState !== "downloading" && updateState !== "installing") {
    checkUpdate({ quiet: false });
  }
});

listen("popover-update-progress", (event) => {
  const { downloaded, total } = event.payload;
  if (total) {
    const percent = Math.min(100, Math.floor((downloaded / total) * 100));
    setUpdateState(percent >= 100 ? "installing" : "downloading", { percent });
  }
});

// 팝오버 창은 앱을 켤 때 숨긴 채 만들어지므로, 처음 읽기에서는 업데이트를 확인하지 않는다.
listen("popover-shown", () => refresh({ checkUpdate: true }));
renderUpdate();
refresh({ checkUpdate: false });

async function refresh({ checkUpdate: shouldCheck }) {
  const [settings, status, logs, info] = await Promise.all([
    invoke("load_settings").catch(() => null),
    invoke("auto_watch_status").catch(() => null),
    invoke("recent_logs", { limit: RECENT_LIMIT }).catch(() => []),
    invoke("app_info").catch(() => null)
  ]);
  renderStatus(settings, status);
  renderRecent(logs);
  renderFolders(settings?.watchFolders ?? []);
  appVersion.textContent = info ? `버전 ${info.version}` : "버전";
  await fitWindow();
  if (shouldCheck && Date.now() - lastUpdateCheck > UPDATE_RECHECK_MS) {
    checkUpdate({ quiet: true });
  }
}

// quiet: 팝오버를 열 때 저절로 하는 확인. 확인 중 표시와 실패 표시를 하지 않는다.
async function checkUpdate({ quiet }) {
  if (["checking", "downloading", "installing"].includes(updateState)) {
    return;
  }
  const previous = updateState;
  if (!quiet) {
    setUpdateState("checking");
  }
  try {
    const version = await invoke("check_app_update");
    lastUpdateCheck = Date.now();
    if (version) {
      setUpdateState("available", { version });
    } else {
      setUpdateState("latest");
    }
  } catch (error) {
    if (String(error).includes("busy")) {
      // 자동 업데이트나 설정 창이 확인·설치 중이다.
      setUpdateState(quiet ? previous : "busy");
      return;
    }
    if (quiet) {
      setUpdateState(previous);
    } else {
      setUpdateState("failed", { error });
    }
  }
}

async function installUpdate() {
  setUpdateState("downloading", { percent: 0 });
  try {
    // 성공하면 앱이 다시 시작되므로 돌아오지 않는다.
    await invoke("install_app_update");
  } catch (error) {
    setUpdateState(String(error).includes("busy") ? "busy" : "failed", { error });
  }
}

function setUpdateState(state, { version, percent, error } = {}) {
  updateState = state;
  if (version) {
    newVersion = version;
  }
  renderUpdate({ percent, error });
}

function renderUpdate({ percent, error } = {}) {
  const view = {
    idle: { icon: "icon-refresh", text: "업데이트 확인", tip: "새 버전이 있는지 확인합니다." },
    checking: { icon: "icon-refresh", tone: "is-spinning", text: "확인 중", tip: "" },
    latest: { icon: "icon-check", tone: "is-success", text: "최신 버전", tip: "눌러서 다시 확인합니다." },
    available: {
      icon: "icon-download",
      tone: "is-accent",
      text: `새 버전 ${newVersion} 업데이트`,
      tip: "눌러서 설치하고 다시 시작합니다."
    },
    downloading: {
      icon: "icon-download",
      tone: "is-accent",
      text: `내려받는 중 ${percent ?? 0}%`,
      tip: ""
    },
    installing: { icon: "icon-download", tone: "is-accent", text: "설치 중", tip: "" },
    busy: { icon: "icon-refresh", tone: "is-spinning", text: "업데이트 진행 중", tip: "눌러서 다시 확인합니다." },
    failed: {
      icon: "icon-info",
      tone: "is-failed",
      text: "확인 실패 · 다시 시도",
      tip: error ? String(error) : ""
    }
  }[updateState];

  updateIconBox.className = `pop-update-icon ${view.tone ?? ""}`.trim();
  updateIcon.className = `icon ${view.icon}`;
  updateText.textContent = view.text;
  updateButton.classList.toggle("is-available", updateState === "available");
  updateButton.disabled = ["checking", "downloading", "installing"].includes(updateState);
  updateButton.title = view.tip;
}

function renderStatus(settings, status) {
  const running = Boolean(status?.running);
  statusDot.classList.toggle("is-on", running);
  if (running) {
    statusText.textContent = `자동 감시 중 · 폴더 ${status.folderCount}개`;
  } else if (settings?.autoWatchEnabled) {
    statusText.textContent = "자동 감시 대기 중 · 켜진 폴더 없음";
  } else {
    statusText.textContent = "자동 감시 꺼짐";
  }
}

function renderRecent(logs) {
  recentList.replaceChildren();
  if (logs.length === 0) {
    recentList.append(emptyRow("아직 정리한 파일이 없습니다."));
    return;
  }
  for (const entry of logs.slice(0, RECENT_LIMIT)) {
    const failed = entry.status === "failed";
    const detail = failed
      ? `실패${entry.reason ? `: ${entry.reason}` : ""}`
      : `${relativeTime(Number(entry.timestampMs))} · ${parentName(entry.finalPath ?? entry.originalPath)}`;
    recentList.append(
      itemRow({
        icon: failed ? "icon-info" : "icon-check",
        tone: failed ? "is-failed" : "is-success",
        title: failed ? entry.originalName : entry.finalName,
        detail,
        tooltip: `${entry.originalName} → ${entry.finalName}`
      })
    );
  }
}

function renderFolders(folders) {
  folderList.replaceChildren();
  if (folders.length === 0) {
    folderList.append(emptyRow("감시 폴더가 없습니다. 설정에서 추가하세요."));
    return;
  }
  for (const folder of folders.slice(0, FOLDER_LIMIT)) {
    const row = itemRow({
      icon: "icon-folder",
      title: lastPathComponent(folder.path),
      detail: shortPath(folder.path),
      tooltip: folder.path
    });
    const badge = document.createElement("span");
    badge.className = `pop-badge${folder.enabled ? " is-on" : ""}`;
    badge.textContent = folder.enabled ? "켬" : "끔";
    row.append(badge);
    folderList.append(row);
  }
  if (folders.length > FOLDER_LIMIT) {
    const more = document.createElement("li");
    more.className = "pop-more";
    more.textContent = `외 ${folders.length - FOLDER_LIMIT}개`;
    folderList.append(more);
  }
}

function itemRow({ icon, tone = "", title, detail, tooltip }) {
  const row = document.createElement("li");
  row.className = "pop-item";
  row.title = tooltip;

  const iconBox = document.createElement("span");
  iconBox.className = `pop-item-icon ${tone}`.trim();
  const glyph = document.createElement("span");
  glyph.className = `icon ${icon}`;
  iconBox.append(glyph);

  const text = document.createElement("span");
  text.className = "pop-item-text";
  const titleEl = document.createElement("span");
  titleEl.className = "pop-item-title";
  titleEl.textContent = title;
  const detailEl = document.createElement("span");
  detailEl.className = "pop-item-detail";
  detailEl.textContent = detail;
  text.append(titleEl, detailEl);

  row.append(iconBox, text);
  return row;
}

function emptyRow(message) {
  const row = document.createElement("li");
  row.className = "pop-empty";
  row.textContent = message;
  return row;
}

async function fitWindow() {
  const height = Math.ceil(panel.getBoundingClientRect().height);
  try {
    await invoke("fit_popover", { height });
  } catch (error) {
    console.warn("failed to fit popover", error);
  }
}

function relativeTime(timestamp) {
  const minutes = Math.floor((Date.now() - timestamp) / 60000);
  if (minutes < 1) {
    return "방금";
  }
  if (minutes < 60) {
    return `${minutes}분 전`;
  }
  const hours = Math.floor(minutes / 60);
  if (hours < 24) {
    return `${hours}시간 전`;
  }
  return new Intl.DateTimeFormat("ko-KR", { month: "numeric", day: "numeric" }).format(
    new Date(timestamp)
  );
}

function lastPathComponent(path) {
  const parts = String(path).split(/[\\/]+/).filter(Boolean);
  return parts.at(-1) ?? String(path);
}

function parentName(path) {
  const parts = String(path).split(/[\\/]+/).filter(Boolean);
  return parts.at(-2) ?? "";
}

// /Users/<이름>/... 와 C:\Users\<이름>\... 는 ~ 로 줄여 보여 준다.
function shortPath(path) {
  return String(path)
    .replace(/^\/Users\/[^/]+/, "~")
    .replace(/^[A-Za-z]:\\Users\\[^\\]+/, "~");
}
