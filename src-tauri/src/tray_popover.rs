// 알림 영역 아이콘의 팝오버. 맥은 메뉴 막대 아이콘을 누르면 아이콘 아래에,
// Windows는 작업 표시줄 아이콘을 오른쪽 클릭하면 아이콘 위에 같은 작은 창을 띄운다.
// 맥: 모서리를 둥글게 하려고 테두리 없는 투명 창을 쓰고, 면과 선은 popover.css가 칠한다.
// Windows: 테두리 없는 창에 그림자를 켜면 Windows 11이 모서리와 테두리를 그린다.
// 다른 곳을 누르면(포커스를 잃으면) 닫힌다.
use std::sync::Mutex;
use std::time::{Duration, Instant};

use tauri::{
    AppHandle, Emitter, Manager, PhysicalPosition, PhysicalSize, Rect, WebviewUrl,
    WebviewWindowBuilder, Window,
};

pub const LABEL: &str = "popover";
const WIDTH: f64 = 320.0;
// 아이콘과 팝오버 사이 간격, 화면 가장자리 여백 (논리 픽셀)
const GAP: f64 = 6.0;
const EDGE: f64 = 8.0;

// 팝오버가 열린 채로 아이콘을 다시 누르면, 클릭보다 포커스 잃음이 먼저 와서 창이 닫힌다.
// 그 직후의 클릭으로 다시 열리지 않게 닫힌 시각을 기억한다.
static LAST_HIDDEN: Mutex<Option<Instant>> = Mutex::new(None);

// 팝오버를 붙일 가장자리 (물리 픽셀 y). 내용 높이가 바뀌어도 이 선에 붙어 있게 한다.
#[derive(Clone, Copy)]
enum Anchor {
    // 메뉴 막대처럼 아이콘이 위에 있으면 팝오버 위쪽을 붙인다.
    Top(f64),
    // 작업 표시줄처럼 아이콘이 아래에 있으면 팝오버 아래쪽을 붙인다.
    Bottom(f64),
}

static ANCHOR: Mutex<Option<Anchor>> = Mutex::new(None);

pub fn create(app: &AppHandle) -> tauri::Result<()> {
    WebviewWindowBuilder::new(app, LABEL, WebviewUrl::App("popover.html".into()))
        .title("한글본드")
        .inner_size(WIDTH, 360.0)
        .decorations(false)
        .transparent(cfg!(target_os = "macos"))
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
    let icon_top = icon_position.y;
    let icon_bottom = icon_position.y + icon_size.height;
    let width = WIDTH * scale;
    let mut x = icon_position.x + icon_size.width / 2.0 - width / 2.0;

    // 화면에서 메뉴 막대·작업 표시줄을 뺀 영역. 모르면 아이콘 위치만 쓴다.
    let work_area = app
        .monitor_from_point(icon_position.x, icon_position.y)
        .ok()
        .flatten()
        .map(|monitor| *monitor.work_area());
    let anchor = match work_area {
        Some(area) => {
            let left = area.position.x as f64 + EDGE * scale;
            let right = area.position.x as f64 + area.size.width as f64 - EDGE * scale;
            x = x.min(right - width).max(left);
            let top = area.position.y as f64;
            let bottom = top + area.size.height as f64;
            if icon_top > top + area.size.height as f64 / 2.0 {
                Anchor::Bottom(icon_top.min(bottom) - GAP * scale)
            } else {
                Anchor::Top(icon_bottom.max(top) + GAP * scale)
            }
        }
        None => Anchor::Top(icon_bottom + GAP * scale),
    };
    if let Ok(mut saved) = ANCHOR.lock() {
        *saved = Some(anchor);
    }
    let height = window
        .outer_size()
        .map(|size| size.height as f64)
        .unwrap_or(0.0);
    let y = match anchor {
        Anchor::Top(top) => top,
        Anchor::Bottom(bottom) => bottom - height,
    };
    let _ = window.set_position(PhysicalPosition::new(x, y));
    let _ = window.show();
    let _ = window.set_focus();
    // 열 때마다 상태·최근 정리·폴더·업데이트를 새로 읽게 한다.
    let _ = window.emit("popover-shown", ());
}

/// 팝오버 내용 높이(논리 픽셀)에 맞춰 창 높이를 바꾼다. 아래쪽에 붙은 팝오버는 위로 자란다.
#[tauri::command]
pub fn fit_popover(window: Window, height: f64) -> Result<(), String> {
    let scale = window.scale_factor().map_err(|error| error.to_string())?;
    let size = PhysicalSize::new((WIDTH * scale).round(), (height * scale).round());
    window.set_size(size).map_err(|error| error.to_string())?;
    let anchor = ANCHOR.lock().ok().and_then(|saved| *saved);
    if let Some(Anchor::Bottom(bottom)) = anchor {
        // Windows는 그림자 테두리만큼 바깥 크기가 더 크다. 위치는 바깥 크기로 맞춘다.
        let outer_height = window
            .outer_size()
            .map(|outer| outer.height as f64)
            .unwrap_or(size.height);
        let x = window
            .outer_position()
            .map(|position| position.x as f64)
            .unwrap_or(0.0);
        let _ = window.set_position(PhysicalPosition::new(x, bottom - outer_height));
    }
    Ok(())
}
