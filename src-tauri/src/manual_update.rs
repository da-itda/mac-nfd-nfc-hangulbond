// 팝오버 맨 아래 업데이트 단추. 맥·Windows 공용.
// 확인과 설치를 Rust에서 한다. 사용자가 다른 곳을 눌러 팝오버가 숨겨져도
// (맥 WKWebView는 숨긴 창의 JS를 멈춘다) 내려받기와 재시작이 끝까지 진행된다.
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;

use tauri::{AppHandle, Emitter};
use tauri_plugin_updater::{Update, UpdaterExt};

use crate::release_runtime_guards;

/// 내려받기 진행을 팝오버에 알리는 이벤트. 값은 { downloaded, total }.
const PROGRESS_EVENT: &str = "popover-update-progress";

// 확인에서 찾은 새 버전. 설치 단추를 누르면 이것을 설치한다.
static FOUND: Mutex<Option<Update>> = Mutex::new(None);
// 확인·설치가 겹치지 않게 한다.
static BUSY: AtomicBool = AtomicBool::new(false);

#[derive(serde::Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct Progress {
    downloaded: u64,
    total: Option<u64>,
}

struct BusyGuard;

impl BusyGuard {
    fn acquire() -> Result<Self, String> {
        if BUSY.swap(true, Ordering::SeqCst) {
            return Err("busy".into());
        }
        Ok(Self)
    }
}

impl Drop for BusyGuard {
    fn drop(&mut self) {
        BUSY.store(false, Ordering::SeqCst);
    }
}

/// 새 버전이 있으면 그 버전 문자열, 최신이면 None.
#[tauri::command]
pub async fn check_app_update(app: AppHandle) -> Result<Option<String>, String> {
    let _busy = BusyGuard::acquire()?;
    let update = app
        .updater()
        .map_err(|error| error.to_string())?
        .check()
        .await
        .map_err(|error| error.to_string())?;
    let version = update.as_ref().map(|update| update.version.clone());
    if let Ok(mut found) = FOUND.lock() {
        *found = update;
    }
    Ok(version)
}

/// 확인에서 찾은 새 버전을 내려받아 설치하고 다시 시작한다.
/// Windows는 설치 파일을 띄운 뒤 업데이터가 앱을 끝내고, 설치 파일이 앱을 다시 켠다.
#[tauri::command]
pub async fn install_app_update(app: AppHandle) -> Result<(), String> {
    let _busy = BusyGuard::acquire()?;
    let update = FOUND
        .lock()
        .ok()
        .and_then(|found| found.clone())
        .ok_or_else(|| "no update to install".to_string())?;

    let mut downloaded: u64 = 0;
    let progress_app = app.clone();
    update
        .download_and_install(
            move |chunk, total| {
                downloaded += chunk as u64;
                let _ = progress_app.emit(PROGRESS_EVENT, Progress { downloaded, total });
            },
            || {},
        )
        .await
        .map_err(|error| error.to_string())?;

    release_runtime_guards(&app)?;
    app.restart();
}
