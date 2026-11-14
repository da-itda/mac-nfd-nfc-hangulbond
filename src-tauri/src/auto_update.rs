// macOS 자동 업데이트.
// 설정 창이 숨겨져 있으면 WKWebView가 페이지의 JS 타이머를 멈춰서, main.js의 자동 확인이
// 창을 열기 전까지 돌지 않는다 (0.2.0에서 확인). 그래서 맥에서는 이 스레드가 확인한다.
// 앱을 켜고 30초 뒤와 6시간마다 확인한다. 설정 창이 보이거나 검사·정리 중이면 설치하지 않고
// 정보 탭에 새 버전을 알린다. Windows는 main.js의 확인이 그대로 동작한다.
use std::thread;
use std::time::{Duration, SystemTime};

use tauri::{async_runtime, AppHandle, Emitter, Manager};
use tauri_plugin_updater::UpdaterExt;

use crate::{release_runtime_guards, AppState};

/// 정보 탭에 새 버전을 알리는 이벤트. 값은 새 버전 문자열.
pub const AVAILABLE_EVENT: &str = "auto-update-available";

const FIRST_CHECK_DELAY: Duration = Duration::from_secs(30);
const CHECK_INTERVAL: Duration = Duration::from_secs(6 * 60 * 60);
// 잠자기 동안 멈추는 단조 시계 대신 벽시계로 간격을 재려고 짧게 나눠 잔다.
const TICK: Duration = Duration::from_secs(5 * 60);
const BUSY_RETRY: Duration = Duration::from_secs(5);

pub fn spawn(app: AppHandle) {
    thread::spawn(move || {
        thread::sleep(FIRST_CHECK_DELAY);
        loop {
            let checked_at = SystemTime::now();
            async_runtime::block_on(check_once(&app));
            while SystemTime::now()
                .duration_since(checked_at)
                .unwrap_or(Duration::ZERO)
                < CHECK_INTERVAL
            {
                thread::sleep(TICK);
            }
        }
    });
}

async fn check_once(app: &AppHandle) {
    let Ok(updater) = app.updater() else {
        return;
    };
    // 네트워크 오류 등은 다음 확인 때 다시 시도한다.
    let Ok(Some(update)) = updater.check().await else {
        return;
    };
    if is_busy(app) {
        let _ = app.emit_to("main", AVAILABLE_EVENT, update.version.clone());
        return;
    }
    if update.download_and_install(|_, _| {}, || {}).await.is_err() {
        return;
    }
    // 내려받는 사이 설정 창을 열었거나 정리를 시작했으면 끝날 때까지 기다렸다가 재시작한다.
    while app
        .state::<AppState>()
        .manual_operation
        .lock()
        .map(|active| active.is_some())
        .unwrap_or(false)
    {
        thread::sleep(BUSY_RETRY);
    }
    if release_runtime_guards(app).is_ok() {
        app.restart();
    }
}

fn is_busy(app: &AppHandle) -> bool {
    let window_visible = app
        .get_webview_window("main")
        .and_then(|window| window.is_visible().ok())
        .unwrap_or(false);
    let operation_running = app
        .state::<AppState>()
        .manual_operation
        .lock()
        .map(|active| active.is_some())
        .unwrap_or(true);
    window_visible || operation_running
}
