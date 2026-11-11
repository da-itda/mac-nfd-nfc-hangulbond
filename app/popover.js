// 맥 메뉴 막대 팝오버: 감시 상태, 최근 정리 3개, 감시 폴더, 설정·종료 단추.
// 열릴 때마다(lib.rs가 popover-shown을 보냄) 새로 읽고, 내용 높이에 맞춰 창 높이를 바꾼다.
const { invoke } = window.__TAURI__.core;
const { listen } = window.__TAURI__.event;
const appWindow = window.__TAURI__.window.getCurrentWindow();
const { LogicalSize } = window.__TAURI__.dpi;

const POPOVER_WIDTH = 320;
const RECENT_LIMIT = 3;
const FOLDER_LIMIT = 3;

const panel = document.querySelector(".popover");
const statusDot = document.querySelector("#status-dot");
const statusText = document.querySelector("#status-text");
const recentList = document.querySelector("#recent-list");
const folderList = document.querySelector("#folder-list");
const appVersion = document.querySelector("#app-version");

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

listen("popover-shown", refresh);
refresh();

async function refresh() {
  const [settings, status, logs, info] = await Promise.all([
    invoke("load_settings").catch(() => null),
    invoke("auto_watch_status").catch(() => null),
    invoke("recent_logs", { limit: RECENT_LIMIT }).catch(() => []),
    invoke("app_info").catch(() => null)
  ]);
  renderStatus(settings, status);
  renderRecent(logs);
  renderFolders(settings?.watchFolders ?? []);
  appVersion.textContent = info ? `버전 ${info.version}` : "";
  await fitWindow();
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
    await appWindow.setSize(new LogicalSize(POPOVER_WIDTH, height));
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

// /Users/<이름>/... 는 ~/... 로 줄여 보여 준다.
function shortPath(path) {
  return String(path).replace(/^\/Users\/[^/]+/, "~");
}
