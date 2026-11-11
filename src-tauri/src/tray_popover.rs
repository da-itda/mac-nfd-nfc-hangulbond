// macOS 메뉴 막대 아이콘을 누르면 기본 메뉴 대신 아이콘 아래에 작은 창(팝오버)을 띄운다.
// 창 모서리를 둥글게 하려고 테두리 없는 투명 창을 쓰고, 면과 선은 popover.css가 칠한다.
// 다른 곳을 누르면(포커스를 잃으면) 닫힌다.
use std::sync::Mutex;
use std::time::{Duration, Instant};

use tauri::{
    AppHandle, Emitter, Manager, PhysicalPosition, Rect, WebviewUrl, WebviewWindowBuilder,
};

pub const LABEL: &str = "popover";
const WIDTH: f64 = 320.0;
// 메뉴 막대 아래 간격과 화면 가장자리 여백 (논리 픽셀)
const GAP: f64 = 6.0;
const EDGE: f64 = 8.0;

// 팝오버가 열린 채로 아이콘을 다시 누르면, 클릭보다 포커스 잃음이 먼저 와서 창이 닫힌다.
// 그 직후의 클릭으로 다시 열리지 않게 닫힌 시각을 기억한다.
static LAST_HIDDEN: Mutex<Option<Instant>> = Mutex::new(None);

pub fn create(app: &AppHandle) -> tauri::Result<()> {
    WebviewWindowBuilder::new(app, LABEL, WebviewUrl::App("popover.html".into()))
        .title("한글본드")
        .inner_size(WIDTH, 360.0)
        .decorations(false)
        .transparent(true)
        .shadow(true)
        .resizable(false)
        .always_on_top(true)
        .skip_taskbar(true)
        .visible(false)
        .build()?;
    Ok(())
}

pub fn hide(app: &AppHandle) {
    if let Some(window) = app.get_webview_window(LABEL) {
        let _ = window.hide();
    }
}

pub fn on_focus_lost(app: &AppHandle) {
    hide(app);
    if let Ok(mut last) = LAST_HIDDEN.lock() {
        *last = Some(Instant::now());
    }
}

pub fn toggle(app: &AppHandle, icon: Rect) {
    let Some(window) = app.get_webview_window(LABEL) else {
        return;
    };
    if window.is_visible().unwrap_or(false) {
        let _ = window.hide();
        return;
    }
    let just_hidden = LAST_HIDDEN
        .lock()
        .ok()
        .and_then(|last| *last)
        .is_some_and(|at| at.elapsed() < Duration::from_millis(300));
    if just_hidden {
        return;
    }

    let scale = window.scale_factor().unwrap_or(1.0);
    let icon_position = icon.position.to_physical::<f64>(scale);
    let icon_size = icon.size.to_physical::<f64>(scale);
    let width = WIDTH * scale;
    let mut x = icon_position.x + icon_size.width / 2.0 - width / 2.0;
    if let Ok(Some(monitor)) = app.monitor_from_point(icon_position.x, icon_position.y) {
        let left = monitor.position().x as f64 + EDGE * scale;
        let right = monitor.position().x as f64 + monitor.size().width as f64 - EDGE * scale;
        x = x.min(right - width).max(left);
    }
    let y = icon_position.y + icon_size.height + GAP * scale;
    let _ = window.set_position(PhysicalPosition::new(x, y));
    let _ = window.show();
    let _ = window.set_focus();
    // 열 때마다 상태·최근 정리·폴더를 새로 읽게 한다.
    let _ = window.emit("popover-shown", ());
}
