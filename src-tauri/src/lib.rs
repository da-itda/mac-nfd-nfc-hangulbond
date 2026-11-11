mod browser_extension;
mod logs;
mod normalizer;
mod os;
mod scanner;
mod settings;
mod stabilization;
mod watcher;

use std::fs;
use std::path::{Path, PathBuf};
#[cfg(target_os = "macos")]
use std::process::Command;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use logs::{append_rename_results, read_recent_logs, LogEntry};
use os::home_dir;
use scanner::{
    normalize_directory_with_progress, normalize_selected_targets, scan_directory_interruptible,
    NormalizeOptions, NormalizeOutcome, NormalizeProgress, RenameResult, ScanOutcome, ScanTarget,
};
use settings::{load_settings_from_file, save_settings_to_file, AppSettings};
use tauri::{
    async_runtime,
    image::Image,
    menu::{Menu, MenuItem, PredefinedMenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    AppHandle, Emitter, Manager, WindowEvent,
};
use tauri_plugin_autostart::ManagerExt;
use watcher::{WatchService, WatchStatus};

const LEGACY_IDENTIFIERS: &[&str] = &["com.codex.hangul-nfc-normalizer", "com.hangulbond.app"];
const AUTOSTART_ENTRY_NAME: &str = "HangulBond";
const WEBSITE_URL: &str = "https://github.com/da-itda/mac-nfd-nfc-hangulbond";
#[cfg(target_os = "macos")]
const LEGACY_AUTOSTART_MARKERS: &[&str] = &[
    "한글본드",
    "한글 파일명 정리기",
    "hangul-nfc-normalizer",
    "com.codex.hangul-nfc-normalizer",
    "com.hangulbond.app",
    "com.hangulbond.desktop",
];

struct AppState {
    watch_service: Mutex<WatchService>,
    instance_lock: Mutex<Option<fs::File>>,
    manual_operation: Mutex<Option<Arc<AtomicBool>>>,
    // 처음 설치해 실행한 경우에만 웹 스토어를 한 번 연다. 업데이트 후 재시작 때는 열지 않는다.
    first_run: AtomicBool,
}

#[derive(Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct AppInfo {
    version: String,
    app_data_dir: String,
    settings_path: String,
    log_path: String,
    launch_at_login: bool,
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct NormalizeProgressEvent {
    scope: String,
    #[serde(flatten)]
    progress: NormalizeProgress,
}

#[tauri::command]
async fn scan_folder(
    app: AppHandle,
    path: String,
    recursive: Option<bool>,
) -> Result<ScanOutcome, String> {
    let cancel = begin_manual_operation(&app)?;
    let root_path = PathBuf::from(path);
    let recursive = recursive.unwrap_or(true);
    let app_for_worker = app.clone();
    let cancel_for_worker = cancel.clone();
    emit_normalize_progress(
        &app,
        "scan",
        NormalizeProgress {
            phase: "scanning".to_string(),
            completed: 0,
            total: 0,
            current_name: None,
            final_name: None,
            status: None,
            reason: None,
        },
    );

    let worker_result = async_runtime::spawn_blocking(move || {
        let outcome =
            scan_directory_interruptible(&root_path, recursive, &cancel_for_worker, |scanned| {
                emit_normalize_progress(
                    &app_for_worker,
                    "scan",
                    NormalizeProgress {
                        phase: "scanning".to_string(),
                        completed: scanned,
                        total: 0,
                        current_name: None,
                        final_name: None,
                        status: None,
                        reason: None,
                    },
                );
            })?;
        emit_normalize_progress(
            &app_for_worker,
            "scan",
            NormalizeProgress {
                phase: if outcome.cancelled {
                    "cancelled"
                } else {
                    "scan-completed"
                }
                .to_string(),
                completed: outcome.scanned_entries,
                total: 0,
                current_name: None,
                final_name: None,
                status: None,
                reason: None,
            },
        );
        Ok::<ScanOutcome, String>(outcome)
    })
    .await;
    finish_manual_operation(&app, &cancel);
    worker_result.map_err(|error| format!("scan worker failed: {error}"))?
}

#[tauri::command]
async fn normalize_selected_folder_items(
    app: AppHandle,
    path: String,
    recursive: Option<bool>,
    selected_targets: Vec<ScanTarget>,
    stabilization_wait_ms: Option<u64>,
) -> Result<NormalizeOutcome, String> {
    let log_path = log_file_path(&app)?;
    let cancel = begin_manual_operation(&app)?;
    let root_path = PathBuf::from(path);
    let selected_paths = selected_targets
        .into_iter()
        .map(|target| target.path)
        .collect::<Vec<_>>();
    let options = NormalizeOptions {
        recursive: recursive.unwrap_or(true),
        dry_run: false,
        stabilization_wait_ms: stabilization_wait_ms.unwrap_or(3_000),
    };
    let app_for_worker = app.clone();
    let cancel_for_worker = cancel.clone();
    emit_normalize_progress(
        &app,
        "normalize",
        NormalizeProgress {
            phase: "preparing".to_string(),
            completed: 0,
            total: 0,
            current_name: None,
            final_name: None,
            status: None,
            reason: None,
        },
    );

    let worker_result = async_runtime::spawn_blocking(move || {
        let outcome = normalize_selected_targets(
            &root_path,
            options.recursive,
            &selected_paths,
            options,
            &cancel_for_worker,
            |progress| emit_normalize_progress(&app_for_worker, "normalize", progress),
        )?;
        append_rename_results(&log_path, &outcome.results)?;
        Ok::<NormalizeOutcome, String>(outcome)
    })
    .await;
    finish_manual_operation(&app, &cancel);
    worker_result.map_err(|error| format!("normalization worker failed: {error}"))?
}

#[tauri::command]
fn cancel_manual_operation(app: AppHandle) -> Result<bool, String> {
    let state = app.state::<AppState>();
    let active = state
        .manual_operation
        .lock()
        .map_err(|_| "failed to lock manual operation".to_string())?;
    if let Some(cancel) = active.as_ref() {
        cancel.store(true, Ordering::Relaxed);
        Ok(true)
    } else {
        Ok(false)
    }
}

fn begin_manual_operation(app: &AppHandle) -> Result<Arc<AtomicBool>, String> {
    let state = app.state::<AppState>();
    let mut active = state
        .manual_operation
        .lock()
        .map_err(|_| "failed to lock manual operation".to_string())?;
    if active.is_some() {
        return Err("이미 검사 또는 정리 작업이 진행 중입니다.".to_string());
    }

    let cancel = Arc::new(AtomicBool::new(false));
    *active = Some(cancel.clone());
    Ok(cancel)
}

fn finish_manual_operation(app: &AppHandle, completed: &Arc<AtomicBool>) {
    if let Ok(mut active) = app.state::<AppState>().manual_operation.lock() {
        if active
            .as_ref()
            .is_some_and(|current| Arc::ptr_eq(current, completed))
        {
            *active = None;
        }
    }
}

#[tauri::command]
fn recent_logs(app: AppHandle, limit: Option<usize>) -> Result<Vec<LogEntry>, String> {
    read_recent_logs(&log_file_path(&app)?, limit.unwrap_or(10))
}

#[tauri::command]
fn load_settings(app: AppHandle) -> Result<AppSettings, String> {
    let mut settings = load_settings_from_file(&settings_file_path(&app)?)?;
    if let Ok(enabled) = launch_at_login_enabled(&app) {
        settings.launch_at_login = enabled;
    }
    Ok(settings)
}

#[tauri::command]
fn save_settings(app: AppHandle, settings: AppSettings) -> Result<AppSettings, String> {
    let mut normalized_settings = settings.normalized();
    sync_launch_at_login(&app, normalized_settings.launch_at_login)?;
    normalized_settings.launch_at_login =
        launch_at_login_enabled(&app).unwrap_or(normalized_settings.launch_at_login);
    save_settings_to_file(&settings_file_path(&app)?, &normalized_settings)?;
    sync_watch_service(&app, &normalized_settings)?;
    Ok(normalized_settings)
}

#[tauri::command]
fn start_auto_watch(app: AppHandle) -> Result<WatchStatus, String> {
    let mut settings = load_settings_from_file(&settings_file_path(&app)?)?;
    settings.auto_watch_enabled = true;
    save_settings_to_file(&settings_file_path(&app)?, &settings)?;
    sync_watch_service(&app, &settings)
}

#[tauri::command]
fn stop_auto_watch(app: AppHandle) -> Result<WatchStatus, String> {
    let mut settings = load_settings_from_file(&settings_file_path(&app)?)?;
    settings.auto_watch_enabled = false;
    save_settings_to_file(&settings_file_path(&app)?, &settings)?;

    let state = app.state::<AppState>();
    let mut watch_service = state
        .watch_service
        .lock()
        .map_err(|_| "failed to lock watch service".to_string())?;
    Ok(watch_service.stop())
}

#[tauri::command]
fn auto_watch_status(app: AppHandle) -> Result<WatchStatus, String> {
    let state = app.state::<AppState>();
    let watch_service = state
        .watch_service
        .lock()
        .map_err(|_| "failed to lock watch service".to_string())?;
    Ok(watch_service.status())
}

#[tauri::command]
async fn normalize_saved_folders(app: AppHandle) -> Result<Vec<RenameResult>, String> {
    let app_for_worker = app.clone();
    async_runtime::spawn_blocking(move || normalize_saved_folders_impl(&app_for_worker))
        .await
        .map_err(|error| format!("normalization worker failed: {error}"))?
}

#[tauri::command]
fn app_info(app: AppHandle) -> Result<AppInfo, String> {
    let app_data_dir = app
        .path()
        .app_data_dir()
        .map_err(|error| format!("failed to resolve app data directory: {error}"))?;
    let settings_path = settings_file_path(&app)?;
    let log_path = log_file_path(&app)?;
    let version = app.package_info().version.to_string();
    let launch_at_login = launch_at_login_enabled(&app).unwrap_or(false);

    Ok(AppInfo {
        version,
        app_data_dir: app_data_dir.display().to_string(),
        settings_path: settings_path.display().to_string(),
        log_path: log_path.display().to_string(),
        launch_at_login,
    })
}

#[tauri::command]
fn open_website() -> Result<(), String> {
    os::open_url(WEBSITE_URL)
}

#[tauri::command]
async fn chrome_extension_status() -> Result<browser_extension::ExtensionState, String> {
    let home = home_dir()?;
    async_runtime::spawn_blocking(move || browser_extension::status(&home, true))
        .await
        .map_err(|error| format!("extension status worker failed: {error}"))
}

#[tauri::command]
async fn register_chrome_extension(
    app: AppHandle,
) -> Result<browser_extension::ExtensionState, String> {
    let home = home_dir()?;
    let state = async_runtime::spawn_blocking(move || browser_extension::register(&home))
        .await
        .map_err(|error| format!("extension register worker failed: {error}"))??;
    let first_run = app.state::<AppState>().first_run.swap(false, Ordering::SeqCst);
    if state == browser_extension::ExtensionState::StoreInstall && first_run {
        browser_extension::open_store()?;
    }
    Ok(state)
}

#[tauri::command]
fn open_extension_store() -> Result<(), String> {
    browser_extension::open_store()
}

#[tauri::command]
fn restart_app(app: AppHandle) -> Result<(), String> {
    release_runtime_guards(&app)?;
    app.restart();
}

fn log_file_path(app: &AppHandle) -> Result<PathBuf, String> {
    let app_data_dir = app
        .path()
        .app_data_dir()
        .map_err(|error| format!("failed to resolve app data directory: {error}"))?;
    Ok(app_data_dir.join("activity.jsonl"))
}

fn settings_file_path(app: &AppHandle) -> Result<PathBuf, String> {
    let app_data_dir = app
        .path()
        .app_data_dir()
        .map_err(|error| format!("failed to resolve app data directory: {error}"))?;
    Ok(app_data_dir.join("settings.json"))
}

fn acquire_single_instance_lock(app: &AppHandle) -> Result<Option<fs::File>, String> {
    let app_data_dir = app
        .path()
        .app_data_dir()
        .map_err(|error| format!("failed to resolve app data directory: {error}"))?;
    fs::create_dir_all(&app_data_dir).map_err(|error| {
        format!(
            "failed to create app data directory {}: {error}",
            app_data_dir.display()
        )
    })?;

    let lock_path = app_data_dir.join("hangulbond.instance.lock");
    let lock_file = fs::OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .open(&lock_path)
        .map_err(|error| format!("failed to open {}: {error}", lock_path.display()))?;

    if try_lock_instance_file(&lock_file)? {
        Ok(Some(lock_file))
    } else {
        Ok(None)
    }
}

// macOS는 flock, Windows는 LockFileEx로 잠근다. 다른 실행본이 잡고 있으면 false.
fn try_lock_instance_file(lock_file: &fs::File) -> Result<bool, String> {
    match lock_file.try_lock() {
        Ok(()) => Ok(true),
        Err(fs::TryLockError::WouldBlock) => Ok(false),
        Err(fs::TryLockError::Error(error)) => {
            Err(format!("failed to lock instance file: {error}"))
        }
    }
}

fn release_runtime_guards(app: &AppHandle) -> Result<(), String> {
    let state = app.state::<AppState>();
    state
        .watch_service
        .lock()
        .map_err(|_| "failed to lock watch service".to_string())?
        .stop();
    *state
        .instance_lock
        .lock()
        .map_err(|_| "failed to lock instance state".to_string())? = None;
    Ok(())
}

fn migrate_legacy_app_data(app: &AppHandle) -> Result<(), String> {
    let app_data_dir = app
        .path()
        .app_data_dir()
        .map_err(|error| format!("failed to resolve app data directory: {error}"))?;
    let Some(parent_dir) = app_data_dir.parent() else {
        return Ok(());
    };

    fs::create_dir_all(&app_data_dir).map_err(|error| {
        format!(
            "failed to create app data directory {}: {error}",
            app_data_dir.display()
        )
    })?;

    for identifier in LEGACY_IDENTIFIERS {
        let legacy_data_dir = parent_dir.join(identifier);
        if legacy_data_dir == app_data_dir || !legacy_data_dir.is_dir() {
            continue;
        }

        copy_if_missing(&legacy_data_dir, &app_data_dir, "settings.json")?;
        copy_if_missing(&legacy_data_dir, &app_data_dir, "activity.jsonl")?;
    }

    Ok(())
}

fn copy_if_missing(
    source_dir: &Path,
    destination_dir: &Path,
    file_name: &str,
) -> Result<(), String> {
    let source = source_dir.join(file_name);
    let destination = destination_dir.join(file_name);

    if !source.is_file() || destination.exists() {
        return Ok(());
    }

    fs::copy(&source, &destination).map_err(|error| {
        format!(
            "failed to migrate {} to {}: {error}",
            source.display(),
            destination.display()
        )
    })?;

    Ok(())
}

fn sync_watch_service(app: &AppHandle, settings: &AppSettings) -> Result<WatchStatus, String> {
    let state = app.state::<AppState>();
    let mut watch_service = state
        .watch_service
        .lock()
        .map_err(|_| "failed to lock watch service".to_string())?;

    if settings.auto_watch_enabled {
        watch_service.start(settings.clone(), log_file_path(app)?, Some(app.clone()))
    } else {
        Ok(watch_service.stop())
    }
}

fn sync_launch_at_login(app: &AppHandle, enabled: bool) -> Result<(), String> {
    let autolaunch = app.autolaunch();
    let current = autolaunch
        .is_enabled()
        .map_err(|error| format!("failed to read login item state: {error}"))?;

    if current == enabled {
        return Ok(());
    }

    if enabled {
        autolaunch
            .enable()
            .map_err(|error| format!("failed to enable launch at login: {error}"))
    } else {
        autolaunch
            .disable()
            .map_err(|error| format!("failed to disable launch at login: {error}"))
    }
}

fn launch_at_login_enabled(app: &AppHandle) -> Result<bool, String> {
    app.autolaunch()
        .is_enabled()
        .map_err(|error| format!("failed to read login item state: {error}"))
}

#[cfg(target_os = "macos")]
fn cleanup_legacy_autostart_entries() -> Result<bool, String> {
    let Some(home_dir) = std::env::var_os("HOME").map(PathBuf::from) else {
        return Ok(false);
    };
    let launch_agents_dir = home_dir.join("Library").join("LaunchAgents");

    if !launch_agents_dir.is_dir() {
        return Ok(false);
    }

    let entries = fs::read_dir(&launch_agents_dir).map_err(|error| {
        format!(
            "failed to read launch agents directory {}: {error}",
            launch_agents_dir.display()
        )
    })?;
    let mut removed_any = false;

    for entry in entries.filter_map(Result::ok) {
        let path = entry.path();

        if path.extension().and_then(|value| value.to_str()) != Some("plist") {
            continue;
        }

        let stem = path
            .file_stem()
            .and_then(|value| value.to_str())
            .unwrap_or("");
        if stem == AUTOSTART_ENTRY_NAME {
            continue;
        }

        let content = fs::read_to_string(&path).unwrap_or_default();
        let is_legacy_hangulbond_entry = LEGACY_AUTOSTART_MARKERS
            .iter()
            .any(|marker| stem.contains(marker) || content.contains(marker));

        if !is_legacy_hangulbond_entry {
            continue;
        }

        unload_launch_agent(&path);
        fs::remove_file(&path)
            .map_err(|error| format!("failed to remove {}: {error}", path.display()))?;
        removed_any = true;
    }

    Ok(removed_any)
}

#[cfg(not(target_os = "macos"))]
fn cleanup_legacy_autostart_entries() -> Result<bool, String> {
    Ok(false)
}

#[cfg(target_os = "macos")]
fn unload_launch_agent(path: &Path) {
    if let Some(uid) = current_uid() {
        let _ = Command::new("launchctl")
            .args([
                "bootout",
                &format!("gui/{uid}"),
                &path.display().to_string(),
            ])
            .output();
    }

    let _ = Command::new("launchctl").arg("unload").arg(path).output();
}

#[cfg(target_os = "macos")]
fn current_uid() -> Option<String> {
    let output = Command::new("id").arg("-u").output().ok()?;

    if !output.status.success() {
        return None;
    }

    let uid = String::from_utf8(output.stdout).ok()?;
    Some(uid.trim().to_string())
}

// Windows에서는 맥에서 보낸 파일을 받아 저장하는 곳(다운로드·바탕화면)을 처음부터 감시한다.
// 바탕화면이 OneDrive로 옮겨져 있어도 Windows 알려진 폴더 API로 실제 경로를 찾는다.
#[cfg(windows)]
fn apply_windows_first_run_defaults(app: &AppHandle, settings: &mut AppSettings) {
    let folders = [app.path().download_dir(), app.path().desktop_dir()];
    for path in folders.into_iter().flatten() {
        if path.is_dir() && !settings.watch_folders.iter().any(|folder| folder.path == path) {
            settings.watch_folders.push(settings::WatchFolder {
                path,
                recursive: true,
                enabled: true,
            });
        }
    }
    settings.auto_watch_enabled = !settings.watch_folders.is_empty();
    settings.launch_at_login = sync_launch_at_login(app, true).is_ok();
}

fn emit_normalize_progress(app: &AppHandle, scope: &str, progress: NormalizeProgress) {
    let _ = app.emit(
        "normalize-progress",
        NormalizeProgressEvent {
            scope: scope.to_string(),
            progress,
        },
    );
}

fn normalize_saved_folders_impl(app: &AppHandle) -> Result<Vec<RenameResult>, String> {
    let settings = load_settings_from_file(&settings_file_path(app)?)?;
    let mut all_results = Vec::new();
    let mut processed_any_folder = false;

    for folder in settings
        .watch_folders
        .iter()
        .filter(|folder| folder.enabled)
    {
        if !folder.path.is_dir() {
            continue;
        }

        processed_any_folder = true;
        let app_for_progress = app.clone();
        let mut results = normalize_directory_with_progress(
            &folder.path,
            NormalizeOptions {
                recursive: folder.recursive,
                dry_run: false,
                stabilization_wait_ms: settings.stabilization_wait_ms,
            },
            |progress| {
                emit_normalize_progress(&app_for_progress, "saved-folders", progress);
            },
        )?;
        all_results.append(&mut results);
    }

    if !processed_any_folder {
        emit_normalize_progress(
            app,
            "saved-folders",
            NormalizeProgress {
                phase: "completed".to_string(),
                completed: 0,
                total: 0,
                current_name: None,
                final_name: None,
                status: None,
                reason: None,
            },
        );
    }

    append_rename_results(&log_file_path(app)?, &all_results)?;
    let _ = app.emit("manual-results", &all_results);
    Ok(all_results)
}

pub fn run() {
    let builder = tauri::Builder::default();
    // Windows는 이미 실행 중일 때 시작 메뉴에서 다시 열면 새 프로세스가 뜬다. 기존 설정 창을 보여 준다.
    // (macOS는 시스템이 실행 중인 앱을 앞으로 가져온다.)
    #[cfg(windows)]
    let builder = builder.plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
        if let Some(window) = app.get_webview_window("main") {
            let _ = window.show();
            let _ = window.unminimize();
            let _ = window.set_focus();
        }
    }));
    builder
        .manage(AppState {
            watch_service: Mutex::new(WatchService::new()),
            instance_lock: Mutex::new(None),
            manual_operation: Mutex::new(None),
            first_run: AtomicBool::new(false),
        })
        .plugin(
            tauri_plugin_autostart::Builder::new()
                .app_name(AUTOSTART_ENTRY_NAME)
                .build(),
        )
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_process::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .setup(|app| {
            #[cfg(target_os = "macos")]
            {
                let _ = app.handle().set_dock_visibility(false);
                let _ = app
                    .handle()
                    .set_activation_policy(tauri::ActivationPolicy::Accessory);
            }

            let Some(instance_lock) = acquire_single_instance_lock(app.handle())? else {
                std::process::exit(0);
            };
            let state = app.state::<AppState>();
            *state
                .instance_lock
                .lock()
                .map_err(|_| "failed to lock instance state".to_string())? = Some(instance_lock);

            let _ = migrate_legacy_app_data(app.handle());
            let legacy_autostart_was_enabled = cleanup_legacy_autostart_entries().unwrap_or(false);

            let scan_now =
                MenuItem::with_id(app, "scan_now", "즉시 확인 및 변경", true, None::<&str>)?;
            let show_settings =
                MenuItem::with_id(app, "show_settings", "한글본드 설정", true, None::<&str>)?;
            let quit = MenuItem::with_id(app, "quit", "한글본드 종료", true, None::<&str>)?;
            let separator = PredefinedMenuItem::separator(app)?;
            let menu = Menu::with_items(app, &[&scan_now, &show_settings, &separator, &quit])?;
            // macOS는 템플릿 아이콘으로 메뉴바 밝기에 맞춰 시스템이 색을 칠한다.
            #[cfg(target_os = "macos")]
            let tray_icon_bytes = include_bytes!("../icons/tray-template.png").as_slice();
            #[cfg(not(target_os = "macos"))]
            let tray_icon_bytes = include_bytes!("../icons/tray.png").as_slice();
            let tray_icon = Image::from_bytes(tray_icon_bytes)?;

            let tray_builder = TrayIconBuilder::with_id("main-tray")
                .icon(tray_icon)
                .icon_as_template(cfg!(target_os = "macos"))
                .tooltip("한글본드")
                .menu(&menu);
            // Windows는 왼쪽 클릭으로 설정 창만 열고, 메뉴는 오른쪽 클릭으로 연다.
            #[cfg(windows)]
            let tray_builder = tray_builder.show_menu_on_left_click(false);
            let _tray = tray_builder
                .on_menu_event(|app, event| match event.id.as_ref() {
                    "scan_now" => {
                        let app_handle = app.clone();
                        std::thread::spawn(move || {
                            if let Err(error) = normalize_saved_folders_impl(&app_handle) {
                                eprintln!("failed to normalize saved folders: {error}");
                            }
                        });

                        if let Some(window) = app.get_webview_window("main") {
                            let _ = window.show();
                            let _ = window.set_focus();
                        }
                    }
                    "show_settings" => {
                        if let Some(window) = app.get_webview_window("main") {
                            let _ = window.show();
                            let _ = window.set_focus();
                        }
                    }
                    "quit" => {
                        app.exit(0);
                    }
                    _ => {}
                })
                .on_tray_icon_event(|tray, event| {
                    if let TrayIconEvent::Click {
                        button: MouseButton::Left,
                        button_state: MouseButtonState::Up,
                        ..
                    } = event
                    {
                        let app = tray.app_handle();
                        if let Some(window) = app.get_webview_window("main") {
                            let _ = window.show();
                            let _ = window.set_focus();
                        }
                    }
                })
                .build(app)?;

            let settings_path = settings_file_path(app.handle())?;
            let is_first_run = !settings_path.exists();
            app.state::<AppState>()
                .first_run
                .store(is_first_run, Ordering::SeqCst);
            let mut settings = load_settings_from_file(&settings_path)?;
            #[cfg(windows)]
            if is_first_run {
                apply_windows_first_run_defaults(app.handle(), &mut settings);
            }
            if legacy_autostart_was_enabled && settings.launch_at_login {
                let _ = sync_launch_at_login(app.handle(), true);
            }
            if let Ok(enabled) = launch_at_login_enabled(app.handle()) {
                settings.launch_at_login = enabled;
            }
            let _ = save_settings_to_file(&settings_path, &settings);
            let _ = sync_watch_service(app.handle(), &settings)?;

            if let Some(window) = app.get_webview_window("main") {
                if is_first_run {
                    let _ = window.show();
                    let _ = window.set_focus();
                } else {
                    let _ = window.hide();
                }
            }

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            scan_folder,
            normalize_selected_folder_items,
            cancel_manual_operation,
            recent_logs,
            load_settings,
            save_settings,
            start_auto_watch,
            stop_auto_watch,
            auto_watch_status,
            normalize_saved_folders,
            app_info,
            open_website,
            chrome_extension_status,
            register_chrome_extension,
            open_extension_store,
            restart_app
        ])
        .on_window_event(|window, event| {
            if let WindowEvent::CloseRequested { api, .. } = event {
                api.prevent_close();
                let _ = window.hide();
            }
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

#[cfg(test)]
mod single_instance_tests {
    use super::try_lock_instance_file;
    use std::fs;

    #[test]
    fn second_lock_attempt_is_rejected() {
        let temp_dir = tempfile::tempdir().expect("create temp dir");
        let lock_path = temp_dir.path().join("hangulbond.instance.lock");
        let first = fs::OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .open(&lock_path)
            .expect("open first lock");
        let second = fs::OpenOptions::new()
            .read(true)
            .write(true)
            .open(&lock_path)
            .expect("open second lock");

        assert!(try_lock_instance_file(&first).expect("first lock"));
        assert!(!try_lock_instance_file(&second).expect("second lock"));
    }
}
