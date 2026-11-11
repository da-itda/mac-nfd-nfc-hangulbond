const { invoke } = window.__TAURI__.core;
const { open } = window.__TAURI__.dialog;
const { listen } = window.__TAURI__.event;
const updater = window.__TAURI__.updater;
const appWindow = window.__TAURI__.window.getCurrentWindow();
const { LogicalSize } = window.__TAURI__.dpi;

// 두 OS 모두 컴피 공용 디자인 기준을 쓴다. 맥은 styles.css의 platform-mac에서 조금 부드럽게 다듬는다.
document.documentElement.classList.add(
  navigator.userAgent.includes("Windows") ? "platform-windows" : "platform-mac"
);

const MAX_WINDOW_HEIGHT = 720;

const selectedFolder = document.querySelector("#selected-folder");
const chooseFolderButton = document.querySelector("#choose-folder");
const removeSelectedFolderButton = document.querySelector("#remove-selected-folder");
const scanFolderButton = document.querySelector("#scan-folder");
const normalizeFolderButton = document.querySelector("#normalize-folder");
const message = document.querySelector("#message");
const targetsList = document.querySelector("#targets");
const showMoreTargetsButton = document.querySelector("#show-more-targets");
const resultBox = document.querySelector(".result-box");
const actionRow = document.querySelector(".action-row");
const resultTitle = document.querySelector("#result-title");
const selectionControls = document.querySelector("#selection-controls");
const logsList = document.querySelector("#logs");
const batchProgress = document.querySelector("#batch-progress");
const progressTitle = document.querySelector("#progress-title");
const progressCount = document.querySelector("#progress-count");
const progressBar = document.querySelector("#progress-bar");
const progressDetail = document.querySelector("#progress-detail");
const cancelOperationButton = document.querySelector("#cancel-operation");
const selectAllTargetsButton = document.querySelector("#select-all-targets");
const clearTargetSelectionButton = document.querySelector("#clear-target-selection");
const folderList = document.querySelector("#folder-list");
const waitTimeSelect = document.querySelector("#wait-time");
const defaultRecursiveInput = document.querySelector("#default-recursive");
const selectedFolderEnabledInput = document.querySelector("#selected-folder-enabled");
const selectedFolderRecursiveInput = document.querySelector("#selected-folder-recursive");
const autoWatchInput = document.querySelector("#auto-watch");
const launchAtLoginInput = document.querySelector("#launch-at-login");
const watchStatus = document.querySelector("#watch-status");
const waitSummary = document.querySelector("#wait-summary");
const folderCount = document.querySelector("#folder-count");
const refreshLogsButton = document.querySelector("#refresh-logs");
const tabsBar = document.querySelector(".tabs");
const tabButtons = document.querySelectorAll("[data-tab]");
const tabPanels = document.querySelectorAll("[data-panel]");
const appVersion = document.querySelector("#app-version");
const appDataDir = document.querySelector("#app-data-dir");
const settingsPath = document.querySelector("#settings-path");
const logPath = document.querySelector("#log-path");
const launchState = document.querySelector("#launch-state");
const websiteLink = document.querySelector("#website-link");
const checkUpdateButton = document.querySelector("#check-update");
const updateState = document.querySelector("#update-state");
const updateProgress = document.querySelector("#update-progress");
const updateProgressBar = document.querySelector("#update-progress-bar");
const updateProgressDetail = document.querySelector("#update-progress-detail");
const extensionState = document.querySelector("#extension-state");
const openExtensionStoreButton = document.querySelector("#open-extension-store");

const AUTO_UPDATE_DELAY_MS = 30 * 1000;
const AUTO_UPDATE_INTERVAL_MS = 6 * 60 * 60 * 1000;
const EXTENSION_LABELS = {
  noChrome: "Chrome 없음",
  installed: "설치됨",
  registered: "Chrome 재시작 후 사용 설정",
  notPublished: "공개 준비 중",
  notRegistered: "설치 안 됨",
  storeInstall: "웹 스토어에서 설치"
};

let folderPath = "";
let lastTargets = [];
let selectedTargetIndices = new Set();
let hasScanResults = false;
let updateInProgress = false;
let manualOperationBusy = false;
let resultMode = "targets";
let visibleResultCount = 100;
let lastResults = [];
let settings = defaultSettings();
let progressHideTimer = null;
let windowFitTimer = null;
let windowFitQueue = Promise.resolve();

chooseFolderButton.addEventListener("click", async () => {
  const selection = await open({
    directory: true,
    multiple: true,
    title: "감시할 폴더 추가"
  });

  const selectedFolders = normalizeDialogSelection(selection);
  if (selectedFolders.length === 0) {
    return;
  }

  const addResult = addWatchFolders(selectedFolders);
  selectFolder(addResult.selectedPath);
  await saveCurrentSettings({ syncWatch: settings.autoWatchEnabled });

  if (addResult.addedCount === 0) {
    message.textContent = "이미 저장된 폴더입니다. 시작을 눌러 검사해보세요.";
    return;
  }

  message.textContent = `${addResult.addedCount}개 폴더를 추가했습니다. 시작을 눌러 검사해보세요.`;
});

waitTimeSelect.addEventListener("change", async () => {
  settings.stabilizationWaitMs = Number(waitTimeSelect.value);
  await saveCurrentSettings();
});

defaultRecursiveInput.addEventListener("change", async () => {
  settings.defaultRecursive = defaultRecursiveInput.checked;
  await saveCurrentSettings();
});

selectedFolderEnabledInput.addEventListener("change", async () => {
  const folder = currentFolder();
  if (!folder) {
    return;
  }

  folder.enabled = selectedFolderEnabledInput.checked;
  await saveCurrentSettings({ syncWatch: settings.autoWatchEnabled });
});

selectedFolderRecursiveInput.addEventListener("change", async () => {
  const folder = currentFolder();
  if (!folder) {
    return;
  }

  folder.recursive = selectedFolderRecursiveInput.checked;
  await saveCurrentSettings({ syncWatch: settings.autoWatchEnabled });
});

autoWatchInput.addEventListener("change", async () => {
  settings.autoWatchEnabled = autoWatchInput.checked;
  await saveCurrentSettings({ syncWatch: true });
});

launchAtLoginInput.addEventListener("change", async () => {
  settings.launchAtLogin = launchAtLoginInput.checked;
  await saveCurrentSettings();
});

refreshLogsButton.addEventListener("click", loadRecentLogs);
websiteLink.addEventListener("click", async (event) => {
  event.preventDefault();

  try {
    await invoke("open_website");
  } catch (error) {
    updateState.textContent = "홈페이지 열기 실패";
    updateState.title = String(error);
  }
});

checkUpdateButton.addEventListener("click", checkForUpdates);
openExtensionStoreButton.addEventListener("click", async () => {
  try {
    await invoke("open_extension_store");
  } catch (error) {
    extensionState.title = String(error);
  }
});
removeSelectedFolderButton.addEventListener("click", async () => {
  await removeSelectedFolder();
});

for (const button of tabButtons) {
  button.addEventListener("click", () => {
    showTab(button.dataset.tab);
  });
}

scanFolderButton.addEventListener("click", async () => {
  if (!folderPath) {
    return;
  }

  await runWithBusyState(scanFolderButton, "검사 중", async () => {
    hideProgress();
    const outcome = await invoke("scan_folder", {
      path: folderPath,
      recursive: currentFolder()?.recursive ?? settings.defaultRecursive
    });
    lastTargets = outcome.targets ?? [];
    selectedTargetIndices = new Set(lastTargets.map((_, index) => index));
    resultMode = "targets";
    hasScanResults = true;

    setResultBoxVisible(true);
    renderSelectedFolderControls();
    renderTargets();
    normalizeFolderButton.disabled = selectedTargetIndices.size === 0;
    message.textContent = outcome.cancelled
      ? `검사를 중단했습니다. 확인된 ${lastTargets.length}개 항목을 선택할 수 있습니다.`
      : lastTargets.length === 0
      ? "정리할 파일명이나 폴더명이 없습니다."
      : `${lastTargets.length}개 항목을 정리할 수 있습니다.`;
  });
});

normalizeFolderButton.addEventListener("click", async () => {
  if (!folderPath || selectedTargetIndices.size === 0) {
    return;
  }

  await runWithBusyState(normalizeFolderButton, "정리 중", async () => {
    const selectedTargets = [...selectedTargetIndices]
      .sort((left, right) => left - right)
      .map((index) => lastTargets[index]);
    showProgress("백그라운드 정리 중", 0, selectedTargets.length);
    progressDetail.textContent = "다른 탭을 이용해도 작업은 계속됩니다. 필요하면 중단할 수 있습니다.";
    const outcome = await invoke("normalize_selected_folder_items", {
      path: folderPath,
      recursive: currentFolder()?.recursive ?? settings.defaultRecursive,
      selectedTargets,
      stabilizationWaitMs: settings.stabilizationWaitMs
    });

    setResultBoxVisible(true);
    renderResults(outcome.results ?? []);
    await loadRecentLogs();
    lastTargets = [];
    selectedTargetIndices.clear();
    updateTargetSelectionControls();
    normalizeFolderButton.disabled = true;
    message.textContent = outcome.cancelled
      ? `정리를 중단했습니다. ${outcome.results?.length ?? 0}개 항목까지 처리했습니다.`
      : `${outcome.results?.length ?? 0}개 선택 항목을 처리했습니다.`;
    hideProgressSoon();
  });
});

cancelOperationButton.addEventListener("click", async () => {
  cancelOperationButton.disabled = true;
  progressDetail.textContent = "현재 항목을 마친 뒤 안전하게 중단합니다.";
  try {
    await invoke("cancel_manual_operation");
  } catch (error) {
    progressDetail.textContent = `중단 요청 실패: ${error}`;
    cancelOperationButton.disabled = false;
  }
});

selectAllTargetsButton.addEventListener("click", () => {
  selectedTargetIndices = new Set(lastTargets.map((_, index) => index));
  renderTargets();
});

clearTargetSelectionButton.addEventListener("click", () => {
  selectedTargetIndices.clear();
  renderTargets();
});

targetsList.addEventListener("change", (event) => {
  const checkbox = event.target.closest("input[data-target-index]");
  if (!checkbox) {
    return;
  }
  const index = Number(checkbox.dataset.targetIndex);
  const target = lastTargets[index];
  if (checkbox.checked) {
    selectedTargetIndices.add(index);
  } else {
    selectedTargetIndices.delete(index);
  }
  if (target?.kind === "directory") {
    lastTargets.forEach((candidate, candidateIndex) => {
      if (candidateIndex === index || isPathWithin(candidate.path, target.path)) {
        if (checkbox.checked) {
          selectedTargetIndices.add(candidateIndex);
        } else {
          selectedTargetIndices.delete(candidateIndex);
        }
      }
    });
    renderTargets();
    return;
  }
  updateTargetSelectionControls();
});

showMoreTargetsButton.addEventListener("click", () => {
  visibleResultCount += 100;
  renderCurrentResults();
});

watchContentHeight();
init();

async function init() {
  settings = await invoke("load_settings");
  applySettingsToUi();
  await loadAppInfo();
  await refreshWatchStatus();
  await loadRecentLogs();
  setupChromeExtension();
  setTimeout(autoUpdate, AUTO_UPDATE_DELAY_MS);
  setInterval(autoUpdate, AUTO_UPDATE_INTERVAL_MS);
  await listen("watcher-results", async (event) => {
    const results = Array.isArray(event.payload) ? event.payload : [];
    if (results.length > 0) {
      renderResults(results);
      showTab("logs");
      message.textContent = `자동 감시로 ${results.length}개 항목을 정리했습니다.`;
      await loadRecentLogs();
    }
  });
  await listen("manual-results", async (event) => {
    const results = Array.isArray(event.payload) ? event.payload : [];
    renderResults(results);
    showTab("logs");
    message.textContent = results.length === 0
      ? "즉시 확인할 항목이 없습니다."
      : `즉시 확인 및 변경으로 ${results.length}개 항목을 정리했습니다.`;
    await loadRecentLogs();
    setButtonsDisabled(false);
    renderSelectedFolderControls();
    hideProgressSoon();
  });
  await listen("normalize-progress", (event) => {
    renderProgressEvent(event.payload);
  });
}

function showTab(tabName) {
  for (const button of tabButtons) {
    button.classList.toggle("active", button.dataset.tab === tabName);
  }

  for (const panel of tabPanels) {
    panel.classList.toggle("active", panel.dataset.panel === tabName);
  }

  if (tabName === "about") {
    refreshExtensionState();
  }
  scheduleWindowFit();
}

// 탭 내용이 바뀌면 창 높이를 내용만큼 맞춘다. 최대 높이를 넘으면 패널 안에서 스크롤한다.
function watchContentHeight() {
  const observer = new ResizeObserver(scheduleWindowFit);
  for (const panel of tabPanels) {
    for (const child of panel.children) {
      observer.observe(child);
    }
  }
  scheduleWindowFit();
}

// requestAnimationFrame은 숨겨진 창에서 멈추므로 타이머로 모아서 실행한다.
function scheduleWindowFit() {
  clearTimeout(windowFitTimer);
  windowFitTimer = setTimeout(() => {
    // 크기 조절이 겹치지 않도록 순서대로 실행한다.
    windowFitQueue = windowFitQueue.then(fitWindowToContent);
  }, 16);
}

async function fitWindowToContent() {
  const panel = document.querySelector(".tab-panel.active");
  if (!panel) {
    return;
  }

  const contentHeight = Math.min(MAX_WINDOW_HEIGHT, Math.ceil(tabsBar.offsetHeight + panel.scrollHeight));
  if (contentHeight === window.innerHeight) {
    return;
  }

  try {
    // macOS에서는 Tauri 창 크기에 제목 표시줄이 포함되어 웹 화면보다 크다. 그 차이를 재서 더한다.
    // 숨겨진 창에서는 window.innerWidth가 틀릴 수 있어 너비는 Tauri 값을 쓴다.
    const scale = await appWindow.scaleFactor();
    const windowSize = (await appWindow.innerSize()).toLogical(scale);
    const chromeHeight = Math.max(0, Math.round(windowSize.height - window.innerHeight));
    await appWindow.setSize(new LogicalSize(windowSize.width, contentHeight + chromeHeight));
  } catch (error) {
    console.warn("failed to fit window height", error);
  }
}

async function loadRecentLogs() {
  const logs = await invoke("recent_logs", { limit: 10 });
  renderLogs(logs);
}

async function loadAppInfo() {
  const info = await invoke("app_info");
  appVersion.textContent = `Version ${info.version}`;
  appDataDir.textContent = info.appDataDir;
  appDataDir.title = info.appDataDir;
  settingsPath.textContent = info.settingsPath;
  settingsPath.title = info.settingsPath;
  logPath.textContent = info.logPath;
  logPath.title = info.logPath;
  launchState.textContent = info.launchAtLogin ? "켜짐" : "꺼짐";
}

async function checkForUpdates() {
  if (!updater?.check) {
    updateState.textContent = "사용할 수 없음";
    updateState.title = "업데이트 플러그인을 불러오지 못했습니다.";
    return;
  }
  if (updateInProgress) {
    return;
  }

  updateInProgress = true;
  checkUpdateButton.disabled = true;
  updateState.textContent = "확인 중";
  updateState.removeAttribute("title");
  hideUpdateProgress();

  try {
    const update = await updater.check();

    if (!update) {
      updateState.textContent = "최신 버전";
      return;
    }

    await installUpdate(update);
  } catch (error) {
    showUpdateError(error);
  } finally {
    updateInProgress = false;
    checkUpdateButton.disabled = false;
  }
}

// 앱을 켠 뒤와 6시간마다 새 버전을 확인한다.
// 설정 창이 닫혀 있고 검사·정리 중이 아니면 바로 설치하고 재시작한다. 아니면 정보 탭에 알린다.
async function autoUpdate() {
  if (!updater?.check || updateInProgress) {
    return;
  }

  updateInProgress = true;
  try {
    const update = await updater.check();
    if (!update) {
      return;
    }

    const busy = !batchProgress.hidden || (await appWindow.isVisible());
    if (busy) {
      updateState.textContent = `새 버전 ${update.version}`;
      updateState.title = "확인을 누르면 설치합니다.";
      return;
    }

    checkUpdateButton.disabled = true;
    await installUpdate(update);
  } catch (error) {
    // 네트워크 오류 등은 다음 확인 때 다시 시도한다.
    updateState.title = String(error);
  } finally {
    updateInProgress = false;
    checkUpdateButton.disabled = false;
  }
}

async function installUpdate(update) {
  updateState.textContent = `새 버전 ${update.version}`;
  showUpdateProgress("다운로드 준비 중", 0, 1);

  let downloaded = 0;
  let contentLength = 0;
  await update.downloadAndInstall((event) => {
    if (event.event === "Started") {
      downloaded = 0;
      contentLength = Number(event.data.contentLength) || 0;
      showUpdateProgress("다운로드 중", 0, contentLength || 1);
      return;
    }

    if (event.event === "Progress") {
      downloaded += Number(event.data.chunkLength) || 0;
      showUpdateProgress("다운로드 중", downloaded, contentLength || downloaded || 1);
      return;
    }

    if (event.event === "Finished") {
      showUpdateProgress("설치 중", contentLength || downloaded || 1, contentLength || downloaded || 1);
    }
  });

  updateState.textContent = "재시작 중";
  await invoke("restart_app");
}

function showUpdateError(error) {
  const errorText = String(error);
  const userMessage = errorText.includes("platforms")
    ? "배포 파일 확인 필요"
    : "업데이트 정보를 확인할 수 없습니다.";
  updateState.textContent = "확인 실패";
  updateState.title = errorText;
  updateProgressDetail.textContent = userMessage;
  updateProgressDetail.title = errorText;
  updateProgress.hidden = false;
}

// 웹 스토어에 공개된 뒤 처음 실행될 때 Chrome에 확장을 등록한다. Chrome이 사용 설정을 묻는다.
async function setupChromeExtension() {
  await refreshExtensionState("register_chrome_extension");
}

async function refreshExtensionState(command = "chrome_extension_status") {
  try {
    const state = await invoke(command);
    extensionState.textContent = EXTENSION_LABELS[state] ?? state;
    extensionState.removeAttribute("title");
    openExtensionStoreButton.hidden = !["registered", "notRegistered", "storeInstall"].includes(state);
  } catch (error) {
    extensionState.textContent = "확인 실패";
    extensionState.title = String(error);
    openExtensionStoreButton.hidden = true;
  }
}

async function saveCurrentSettings(options = {}) {
  settings = await invoke("save_settings", { settings });
  applySettingsToUi();
  await loadAppInfo();

  if (options.syncWatch) {
    await syncAutoWatch();
  } else {
    await refreshWatchStatus();
  }
}

async function syncAutoWatch() {
  const status = settings.autoWatchEnabled
    ? await invoke("start_auto_watch")
    : await invoke("stop_auto_watch");
  renderWatchStatus(status);
}

async function refreshWatchStatus() {
  const status = await invoke("auto_watch_status");
  renderWatchStatus(status);
}

function renderWatchStatus(status) {
  if (status.running) {
    watchStatus.textContent = `자동 감시 중 (${status.folderCount}개)`;
    return;
  }

  watchStatus.textContent = settings.autoWatchEnabled
    ? "감시할 폴더 없음"
    : "수동 검사 우선";
}

function applySettingsToUi() {
  settings = normalizeSettings(settings);
  waitTimeSelect.value = String(settings.stabilizationWaitMs);
  defaultRecursiveInput.checked = settings.defaultRecursive;
  autoWatchInput.checked = settings.autoWatchEnabled;
  launchAtLoginInput.checked = settings.launchAtLogin;
  waitSummary.textContent = `${settings.stabilizationWaitMs / 1000}초`;
  folderCount.textContent = `${settings.watchFolders.length}개`;

  if (!folderPath && settings.watchFolders.length > 0) {
    folderPath = settings.watchFolders[0].path;
  } else if (folderPath) {
    const stillExists = settings.watchFolders.some((folder) => folder.path === folderPath);
    if (!stillExists) {
      folderPath = settings.watchFolders[0]?.path ?? "";
    }
  }

  renderFolderList();
  renderSelectedFolderControls();
}

function renderFolderList() {
  if (settings.watchFolders.length === 0) {
    folderList.innerHTML = emptyFolderRows().join("");
    return;
  }

  const rows = settings.watchFolders.map((folder, index) => {
    const recursiveLabel = folder.recursive ? "하위 폴더 포함" : "현재 폴더만";
    const enabledLabel = folder.enabled ? "감시 켬" : "감시 끔";
    const enabledClass = folder.enabled ? "is-enabled" : "is-disabled";
    const selectedRowClass = folder.path === folderPath ? " is-selected" : "";
    return `
      <li class="folder-row ${enabledClass}${selectedRowClass}" data-index="${index}">
        <div class="folder-main">
          <span class="folder-name">${escapeHtml(lastPathComponent(folder.path))}</span>
          <span class="folder-meta">${escapeHtml(folder.path)}</span>
          <span class="folder-badges">
            <span>${escapeHtml(enabledLabel)}</span>
            <span>${escapeHtml(recursiveLabel)}</span>
          </span>
        </div>
      </li>
    `;
  });

  folderList.innerHTML = rows.concat(emptyFolderRows(Math.max(0, 3 - rows.length))).join("");
}

folderList.addEventListener("click", async (event) => {
  const row = event.target.closest(".folder-row[data-index]");
  if (!row) {
    return;
  }

  const index = Number(row.dataset.index);
  const folder = settings.watchFolders[index];

  if (!folder) {
    return;
  }

  selectFolder(folder.path);
});

function selectFolder(path, options = {}) {
  folderPath = path;
  lastTargets = [];
  selectedTargetIndices.clear();
  hasScanResults = false;
  selectedFolder.textContent = folderPath;
  scanFolderButton.disabled = false;
  normalizeFolderButton.disabled = true;
  setResultBoxVisible(false);
  renderTargets([]);
  renderSelectedFolderControls();
  if (options.render !== false) {
    renderFolderList();
  }
}

function currentFolder() {
  return settings.watchFolders.find((folder) => folder.path === folderPath);
}

function normalizeDialogSelection(selection) {
  if (!selection) {
    return [];
  }

  return Array.isArray(selection) ? selection : [selection];
}

function addWatchFolders(paths) {
  let addedCount = 0;
  let selectedPath = paths[0] ?? "";

  for (const path of paths) {
    const exists = settings.watchFolders.some((folder) => folder.path === path);
    if (exists) {
      continue;
    }

    settings.watchFolders.push({
      path,
      recursive: settings.defaultRecursive,
      enabled: true
    });
    addedCount += 1;
    selectedPath = path;
  }

  return { addedCount, selectedPath };
}

async function removeSelectedFolder() {
  if (!folderPath) {
    return;
  }

  const index = settings.watchFolders.findIndex((folder) => folder.path === folderPath);
  if (index === -1) {
    return;
  }

  settings.watchFolders.splice(index, 1);
  folderPath = settings.watchFolders[Math.max(0, index - 1)]?.path ?? settings.watchFolders[0]?.path ?? "";
  lastTargets = [];
  hasScanResults = false;
  setResultBoxVisible(false);
  renderTargets([]);
  normalizeFolderButton.disabled = true;
  await saveCurrentSettings({ syncWatch: settings.autoWatchEnabled });
  message.textContent = folderPath
    ? "선택한 폴더를 삭제했습니다."
    : "저장된 폴더가 없습니다.";
}

function renderSelectedFolderControls() {
  const folder = currentFolder();
  const hasFolder = Boolean(folder);

  selectedFolder.textContent = folder?.path ?? "선택된 폴더가 없습니다.";
  selectedFolderEnabledInput.checked = folder?.enabled ?? false;
  selectedFolderRecursiveInput.checked = folder?.recursive ?? false;
  selectedFolderEnabledInput.disabled = !hasFolder;
  selectedFolderRecursiveInput.disabled = !hasFolder;
  removeSelectedFolderButton.disabled = !hasFolder;
  actionRow.hidden = hasScanResults;
  selectionControls.hidden = !hasScanResults;
  resultTitle.textContent = resultMode === "results" ? "정리 결과" : "검사";
  scanFolderButton.disabled = !hasFolder || hasScanResults;
  normalizeFolderButton.disabled = !hasFolder || !hasScanResults || selectedTargetIndices.size === 0;
}

function emptyFolderRows(count = 5) {
  return Array.from({ length: count }, () => "<li class=\"empty-row\"></li>");
}

function setResultBoxVisible(visible) {
  resultBox.classList.toggle("visible", visible);
}

async function runWithBusyState(button, busyText, callback) {
  const originalText = button.textContent;
  manualOperationBusy = true;
  setButtonsDisabled(true);
  button.textContent = busyText;
  cancelOperationButton.disabled = false;

  try {
    await callback();
  } catch (error) {
    message.textContent = `오류: ${error}`;
  } finally {
    manualOperationBusy = false;
    button.textContent = originalText;
    setButtonsDisabled(false);
    scanFolderButton.disabled = !folderPath;
    updateTargetSelectionControls();
    cancelOperationButton.disabled = true;
    renderSelectedFolderControls();
  }
}

function renderProgressEvent(payload) {
  if (!payload || !payload.phase) {
    return;
  }

  const total = Number(payload.total) || 0;
  const completed = Number(payload.completed) || 0;

  if (payload.phase === "scanning") {
    showProgress("백그라운드 검사 중", completed, 0);
    progressCount.textContent = `${completed}개 확인`;
    progressDetail.textContent = "다른 탭을 이용해도 검사는 계속됩니다.";
    return;
  }

  if (payload.phase === "preparing") {
    showProgress("정리 대상 확인 중", completed, 0);
    progressCount.textContent = `${completed}개 확인`;
    progressDetail.textContent = "선택 항목을 확인하고 있습니다. 다른 탭을 이용할 수 있습니다.";
    return;
  }

  if (payload.phase === "scan-completed") {
    showProgress("검사 완료", completed, 0);
    progressCount.textContent = `${completed}개 확인`;
    progressDetail.textContent = "검사 결과에서 정리할 파일이나 폴더를 선택하세요.";
    cancelOperationButton.disabled = true;
    hideProgressSoon(1600);
    return;
  }

  if (payload.phase === "started") {
    setButtonsDisabled(true);
    showProgress(total === 0 ? "정리할 항목 없음" : "백그라운드 정리 중", completed, total);
    progressDetail.textContent = total === 0
      ? "정리할 파일명이나 폴더명이 없습니다."
      : "다른 탭을 이용해도 작업은 계속됩니다.";
    return;
  }

  if (payload.phase === "item") {
    showProgress("백그라운드 정리 중", completed, total);
    const currentName = payload.currentName ?? "";
    const finalName = payload.finalName ?? "";
    const status = statusLabel(payload.status);
    const suffix = payload.reason ? ` · ${payload.reason}` : "";
    const detail = finalName
      ? `${status}: ${currentName} → ${finalName}${suffix}`
      : `${status}: ${currentName}${suffix}`;
    progressDetail.textContent = detail;
    progressDetail.title = detail;
    return;
  }

  if (payload.phase === "completed") {
    showProgress("정리 완료", total, total);
    progressDetail.textContent = total === 0
      ? "정리할 항목이 없습니다."
      : "모든 항목 처리가 끝났습니다.";
    hideProgressSoon();
    cancelOperationButton.disabled = true;
    return;
  }

  if (payload.phase === "cancelled") {
    showProgress("중단됨", completed, total);
    progressDetail.textContent = payload.scope === "scan"
      ? "확인된 항목만 표시합니다. 선택한 항목을 정리할 수 있습니다."
      : "현재까지 처리한 항목을 저장했습니다.";
    cancelOperationButton.disabled = true;
    hideProgressSoon(5000);
  }
}

function showProgress(title, completed, total) {
  if (progressHideTimer) {
    clearTimeout(progressHideTimer);
    progressHideTimer = null;
  }

  const normalizedTotal = Math.max(Number(total) || 0, 0);
  const normalizedCompleted = Math.min(Math.max(Number(completed) || 0, 0), normalizedTotal);
  batchProgress.hidden = false;
  progressTitle.textContent = title;
  progressCount.textContent = `${normalizedCompleted} / ${normalizedTotal}`;
  progressBar.max = Math.max(normalizedTotal, 1);
  progressBar.value = normalizedTotal === 0 ? 0 : normalizedCompleted;
}

function hideProgressSoon(delay = 700) {
  if (progressHideTimer) {
    clearTimeout(progressHideTimer);
  }

  progressHideTimer = setTimeout(() => {
    hideProgress();
    if (!manualOperationBusy) {
      setButtonsDisabled(false);
      renderSelectedFolderControls();
    }
  }, delay);
}

function hideProgress() {
  if (progressHideTimer) {
    clearTimeout(progressHideTimer);
    progressHideTimer = null;
  }

  batchProgress.hidden = true;
  progressTitle.textContent = "정리 중";
  progressCount.textContent = "0 / 0";
  progressBar.max = 1;
  progressBar.value = 0;
  progressDetail.textContent = "항목을 준비하고 있습니다.";
  progressDetail.removeAttribute("title");
}

function showUpdateProgress(label, completed, total) {
  const normalizedTotal = Math.max(Number(total) || 1, 1);
  const normalizedCompleted = Math.min(Math.max(Number(completed) || 0, 0), normalizedTotal);
  updateProgress.hidden = false;
  updateProgressBar.max = normalizedTotal;
  updateProgressBar.value = normalizedCompleted;
  updateProgressDetail.textContent = label;
  updateProgressDetail.title = label;
}

function hideUpdateProgress() {
  updateProgress.hidden = true;
  updateProgressBar.max = 1;
  updateProgressBar.value = 0;
  updateProgressDetail.textContent = "대기 중";
  updateProgressDetail.removeAttribute("title");
}

function setButtonsDisabled(disabled) {
  chooseFolderButton.disabled = disabled;
  removeSelectedFolderButton.disabled = disabled || !folderPath;
  scanFolderButton.disabled = disabled;
  normalizeFolderButton.disabled = disabled || selectedTargetIndices.size === 0;
  selectAllTargetsButton.disabled = disabled || lastTargets.length === 0 || selectedTargetIndices.size === lastTargets.length;
  clearTargetSelectionButton.disabled = disabled || selectedTargetIndices.size === 0;
  selectedFolderRecursiveInput.disabled = disabled || !folderPath;
  for (const checkbox of targetsList.querySelectorAll("input[data-target-index]")) {
    checkbox.disabled = disabled;
  }
}

function renderTargets(targets) {
  if (targets) {
    lastTargets = targets;
    selectedTargetIndices = new Set(lastTargets.map((_, index) => index));
    resultMode = "targets";
    visibleResultCount = 100;
  }

  if (lastTargets.length === 0) {
    targetsList.innerHTML = "<li>정리할 항목이 없습니다.</li>";
    showMoreTargetsButton.hidden = true;
    updateTargetSelectionControls();
    return;
  }

  renderCurrentResults();
  updateTargetSelectionControls();
}

function renderCurrentResults() {
  if (resultMode === "results") {
    renderResultRows();
    return;
  }

  targetsList.innerHTML = lastTargets.slice(0, visibleResultCount).map((target, index) => {
    const status = target.collision ? "충돌" : "변환";
    const kind = target.kind === "directory" ? "폴더" : "파일";
    const parent = target.parentPath ?? "";
    const checked = selectedTargetIndices.has(index) ? "checked" : "";
    return `<li class="target-row">
      <label class="target-check"><input type="checkbox" data-target-index="${index}" ${checked} />
        <span class="target-description"><span class="target-change">${escapeHtml(kind)} · ${escapeHtml(status)}: ${escapeHtml(target.originalName)} → ${escapeHtml(target.finalName)}</span>
        <span class="target-parent" title="${escapeHtml(parent)}">${escapeHtml(parent)}</span></span>
      </label>
    </li>`;
  }).join("");
  showMoreTargetsButton.hidden = visibleResultCount >= lastTargets.length;
  showMoreTargetsButton.textContent = `다음 ${Math.min(100, lastTargets.length - visibleResultCount)}개 표시 (${Math.min(visibleResultCount, lastTargets.length)} / ${lastTargets.length})`;
}

function updateTargetSelectionControls() {
  selectAllTargetsButton.disabled = manualOperationBusy || lastTargets.length === 0 || selectedTargetIndices.size === lastTargets.length;
  clearTargetSelectionButton.disabled = manualOperationBusy || selectedTargetIndices.size === 0;
  normalizeFolderButton.disabled = manualOperationBusy || selectedTargetIndices.size === 0;
}

function isPathWithin(candidatePath, directoryPath) {
  const candidate = String(candidatePath ?? "");
  const directory = String(directoryPath ?? "");
  if (!candidate || !directory) {
    return false;
  }
  const separator = directory.includes("\\") && !directory.includes("/") ? "\\" : "/";
  const prefix = directory.endsWith(separator) ? directory : `${directory}${separator}`;
  return candidate.startsWith(prefix);
}

function renderResults(results) {
  setResultBoxVisible(true);
  hasScanResults = false;
  lastTargets = [];
  selectedTargetIndices.clear();
  lastResults = results;
  resultMode = "results";
  visibleResultCount = 100;
  renderResultRows();
  renderSelectedFolderControls();
  updateTargetSelectionControls();
}

function renderResultRows() {
  if (lastResults.length === 0) {
    targetsList.innerHTML = "<li>처리한 항목이 없습니다.</li>";
    showMoreTargetsButton.hidden = true;
    return;
  }

  targetsList.innerHTML = lastResults.slice(0, visibleResultCount).map((result) => {
    const status = statusLabel(result.status);
    const detail = result.reason ? ` (${result.reason})` : "";
    return `<li>${escapeHtml(status)}: ${escapeHtml(result.originalName)} → ${escapeHtml(result.finalName)}${escapeHtml(detail)}</li>`;
  }).join("");
  showMoreTargetsButton.hidden = visibleResultCount >= lastResults.length;
  showMoreTargetsButton.textContent = `다음 ${Math.min(100, lastResults.length - visibleResultCount)}개 표시 (${Math.min(visibleResultCount, lastResults.length)} / ${lastResults.length})`;
}

function renderLogs(logs) {
  if (logs.length === 0) {
    logsList.innerHTML = "<li>아직 처리한 파일이 없습니다.</li>";
    return;
  }

  logsList.innerHTML = logs.map((entry) => {
    const date = new Date(Number(entry.timestampMs));
    const time = formatLogTime(date);
    const fullTime = date.toLocaleString("ko-KR");
    const message = logMessage(entry);
    const change = `${entry.originalName} → ${entry.finalName}`;
    const fullText = `${fullTime} · ${message} · ${change}`;

    return `
      <li class="log-row" title="${escapeHtml(fullText)}">
        <span class="log-time">${escapeHtml(time)}</span>
        <span class="log-message">${escapeHtml(message)}</span>
        <span class="log-change">${escapeHtml(change)}</span>
      </li>
    `;
  }).join("");
}

function formatLogTime(date) {
  return new Intl.DateTimeFormat("ko-KR", {
    month: "numeric",
    day: "numeric",
    hour: "numeric",
    minute: "2-digit"
  }).format(date);
}

function logMessage(entry) {
  const label = statusLabel(entry.status);
  return entry.reason ? `${label}: ${entry.reason}` : label;
}

function statusLabel(status) {
  switch (status) {
    case "success":
      return "성공";
    case "collision":
      return "충돌 처리";
    case "failed":
      return "실패";
    case "dry-run":
      return "검사";
    default:
      return status ?? "처리";
  }
}

function escapeHtml(value) {
  return String(value)
    .replaceAll("&", "&amp;")
    .replaceAll("<", "&lt;")
    .replaceAll(">", "&gt;")
    .replaceAll('"', "&quot;")
    .replaceAll("'", "&#039;");
}

// Windows 경로는 역슬래시로 나뉜다.
function lastPathComponent(path) {
  const normalized = String(path).replace(/[\\/]+$/, "");
  const parts = normalized.split(/[\\/]/);
  return parts.at(-1) || normalized;
}

function defaultSettings() {
  return {
    watchFolders: [],
    autoWatchEnabled: false,
    launchAtLogin: false,
    stabilizationWaitMs: 3000,
    defaultRecursive: true
  };
}

function normalizeSettings(value) {
  return {
    ...defaultSettings(),
    ...value,
    watchFolders: Array.isArray(value?.watchFolders) ? value.watchFolders : []
  };
}
