use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, RecvTimeoutError, Sender};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use notify::{Config, Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use serde::Serialize;
use tauri::{AppHandle, Emitter};

use crate::logs::append_rename_results;
use crate::scanner::{normalize_directory, normalize_paths, NormalizeOptions, RenameResult};
use crate::settings::{AppSettings, WatchFolder};

const DEBOUNCE_DURATION: Duration = Duration::from_secs(3);
const RECENT_CONVERSION_IGNORE_DURATION: Duration = Duration::from_secs(10);
const SWEEP_INTERVAL: Duration = Duration::from_secs(5);
const MAX_SWEEP_INTERVAL: Duration = Duration::from_secs(60);
// 주기 검사가 CPU를 2% 넘게 쓰지 않도록, 한 번 검사에 걸린 시간의 50배를 다음 간격으로 삼는다.
const SWEEP_IDLE_FACTOR: u32 = 50;

// 감시 폴더가 아주 크면 검사 간격을 늘린다. 이벤트는 그대로 바로 처리하므로 놓친 변경만 조금 늦게 잡힌다.
fn next_sweep_interval(sweep_elapsed: Duration) -> Duration {
    sweep_elapsed
        .saturating_mul(SWEEP_IDLE_FACTOR)
        .clamp(SWEEP_INTERVAL, MAX_SWEEP_INTERVAL)
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct WatchStatus {
    pub running: bool,
    pub folder_count: usize,
}

#[derive(Debug)]
pub struct WatchService {
    stop_tx: Option<Sender<()>>,
    handle: Option<JoinHandle<()>>,
    folder_count: usize,
}

impl WatchService {
    pub fn new() -> Self {
        Self {
            stop_tx: None,
            handle: None,
            folder_count: 0,
        }
    }

    pub fn start(
        &mut self,
        settings: AppSettings,
        log_file_path: PathBuf,
        app_handle: Option<AppHandle>,
    ) -> Result<WatchStatus, String> {
        self.stop();

        let folders = enabled_existing_folders(&settings.watch_folders);
        let folder_count = folders.len();

        if !settings.auto_watch_enabled || folders.is_empty() {
            self.folder_count = 0;
            return Ok(self.status());
        }

        let (stop_tx, stop_rx) = mpsc::channel::<()>();
        let (ready_tx, ready_rx) = mpsc::channel::<()>();
        let (event_tx, event_rx) = mpsc::channel::<notify::Result<Event>>();
        let stabilization_wait_ms = settings.stabilization_wait_ms;

        let handle = thread::spawn(move || {
            let mut watcher = match RecommendedWatcher::new(
                move |result| {
                    let _ = event_tx.send(result);
                },
                Config::default(),
            ) {
                Ok(watcher) => watcher,
                Err(error) => {
                    eprintln!("failed to create watcher: {error}");
                    return;
                }
            };

            for folder in &folders {
                let mode = if folder.recursive {
                    RecursiveMode::Recursive
                } else {
                    RecursiveMode::NonRecursive
                };

                if let Err(error) = watcher.watch(&folder.path, mode) {
                    eprintln!("failed to watch {}: {error}", folder.path.display());
                }
            }

            let _ = ready_tx.send(());

            run_watch_loop(
                event_rx,
                stop_rx,
                log_file_path,
                app_handle,
                folders,
                NormalizeOptions {
                    recursive: true,
                    dry_run: false,
                    stabilization_wait_ms,
                },
            );
        });

        self.stop_tx = Some(stop_tx);
        self.handle = Some(handle);
        self.folder_count = folder_count;
        let _ = ready_rx.recv_timeout(Duration::from_secs(2));
        Ok(self.status())
    }

    pub fn stop(&mut self) -> WatchStatus {
        if let Some(stop_tx) = self.stop_tx.take() {
            let _ = stop_tx.send(());
        }

        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }

        self.folder_count = 0;
        self.status()
    }

    pub fn status(&self) -> WatchStatus {
        WatchStatus {
            running: self.handle.is_some(),
            folder_count: self.folder_count,
        }
    }
}

impl Default for WatchService {
    fn default() -> Self {
        Self::new()
    }
}

fn run_watch_loop(
    event_rx: mpsc::Receiver<notify::Result<Event>>,
    stop_rx: mpsc::Receiver<()>,
    log_file_path: PathBuf,
    app_handle: Option<AppHandle>,
    folders: Vec<WatchFolder>,
    options: NormalizeOptions,
) {
    let mut pending = PendingEvents::new(DEBOUNCE_DURATION);
    let mut recent_conversions = RecentConversions::new(RECENT_CONVERSION_IGNORE_DURATION);
    let mut reported_failures = ReportedFailures::default();
    let mut last_sweep = Instant::now();
    let mut sweep_interval = SWEEP_INTERVAL;

    loop {
        if stop_rx.try_recv().is_ok() {
            return;
        }

        match event_rx.recv_timeout(Duration::from_millis(250)) {
            Ok(Ok(event)) => {
                for path in paths_from_event(event) {
                    if !recent_conversions.should_ignore(&path) {
                        pending.insert(path);
                    }
                }
            }
            Ok(Err(error)) => {
                eprintln!("watch error: {error}");
            }
            Err(RecvTimeoutError::Timeout) => {}
            Err(RecvTimeoutError::Disconnected) => return,
        }

        let ready = pending
            .ready_paths()
            .into_iter()
            .filter(|path| !recent_conversions.should_ignore(path))
            .collect::<Vec<_>>();
        if !ready.is_empty() {
            match normalize_paths(&ready, options) {
                Ok(results) => {
                    handle_results(
                        &results,
                        &log_file_path,
                        app_handle.as_ref(),
                        &mut recent_conversions,
                        &mut reported_failures,
                    );
                }
                Err(error) => {
                    eprintln!("failed to normalize watched paths: {error}");
                }
            }
        }

        if last_sweep.elapsed() >= sweep_interval {
            let sweep_started = Instant::now();
            let mut sweep_found_targets = false;
            for folder in &folders {
                match normalize_directory(
                    &folder.path,
                    NormalizeOptions {
                        recursive: folder.recursive,
                        ..options
                    },
                ) {
                    Ok(results) => {
                        sweep_found_targets |= !results.is_empty();
                        handle_results(
                            &results,
                            &log_file_path,
                            app_handle.as_ref(),
                            &mut recent_conversions,
                            &mut reported_failures,
                        );
                    }
                    Err(error) => {
                        eprintln!(
                            "failed to sweep watched folder {}: {error}",
                            folder.path.display()
                        );
                    }
                }
            }
            reported_failures.forget_missing();
            // 정리할 파일이 있었으면 저장 완료 대기(수 초)가 검사 시간에 섞인다.
            // 그 시간으로 간격을 늘리면 바로 뒤에 다시 저장된 파일이 최대 1분 늦게 정리되므로,
            // 간격은 정리할 것이 없던 순수 검사 시간으로만 정한다.
            sweep_interval = if sweep_found_targets {
                SWEEP_INTERVAL
            } else {
                next_sweep_interval(sweep_started.elapsed())
            };
            last_sweep = Instant::now();
        }
    }
}

fn handle_results(
    results: &[RenameResult],
    log_file_path: &Path,
    app_handle: Option<&AppHandle>,
    recent_conversions: &mut RecentConversions,
    reported_failures: &mut ReportedFailures,
) {
    for result in results {
        if let Some(final_path) = &result.final_path {
            recent_conversions.mark(final_path.clone());
        }
        recent_conversions.mark(result.original_path.clone());
    }

    let results = results
        .iter()
        .filter(|result| reported_failures.should_report(result))
        .cloned()
        .collect::<Vec<_>>();
    if results.is_empty() {
        return;
    }

    if let Err(error) = append_rename_results(log_file_path, &results) {
        eprintln!("failed to append watch log: {error}");
    }

    if let Some(app) = app_handle {
        let _ = app.emit("watcher-results", &results);
    }
}

// 열려 있는 파일처럼 같은 이유로 계속 실패하는 항목은 주기 검사마다 다시 시도하지만 기록은 한 번만 남긴다.
#[derive(Debug, Default)]
struct ReportedFailures {
    reasons: HashMap<PathBuf, Option<String>>,
}

impl ReportedFailures {
    fn should_report(&mut self, result: &RenameResult) -> bool {
        if result.status != "failed" {
            self.reasons.remove(&result.original_path);
            return true;
        }
        let previous = self
            .reasons
            .insert(result.original_path.clone(), result.reason.clone());
        previous.as_ref() != Some(&result.reason)
    }

    fn forget_missing(&mut self) {
        self.reasons.retain(|path, _| path.exists());
    }
}

fn paths_from_event(event: Event) -> Vec<PathBuf> {
    match event.kind {
        EventKind::Create(_) | EventKind::Modify(_) => event
            .paths
            .into_iter()
            .filter(|path| path.file_name().is_some())
            .collect(),
        _ => Vec::new(),
    }
}

fn enabled_existing_folders(folders: &[WatchFolder]) -> Vec<WatchFolder> {
    folders
        .iter()
        .filter(|folder| folder.enabled && folder.path.is_dir())
        .cloned()
        .collect()
}

#[derive(Debug)]
struct PendingEvents {
    debounce_duration: Duration,
    paths: HashMap<PathBuf, Instant>,
}

impl PendingEvents {
    fn new(debounce_duration: Duration) -> Self {
        Self {
            debounce_duration,
            paths: HashMap::new(),
        }
    }

    fn insert(&mut self, path: PathBuf) {
        self.paths.insert(path, Instant::now());
    }

    fn ready_paths(&mut self) -> Vec<PathBuf> {
        let now = Instant::now();
        let ready = self
            .paths
            .iter()
            .filter_map(|(path, last_seen)| {
                if now.duration_since(*last_seen) >= self.debounce_duration {
                    Some(path.clone())
                } else {
                    None
                }
            })
            .collect::<Vec<_>>();

        for path in &ready {
            self.paths.remove(path);
        }

        ready
    }
}

#[derive(Debug)]
struct RecentConversions {
    ignore_duration: Duration,
    paths: HashMap<PathBuf, Instant>,
}

impl RecentConversions {
    fn new(ignore_duration: Duration) -> Self {
        Self {
            ignore_duration,
            paths: HashMap::new(),
        }
    }

    fn mark(&mut self, path: PathBuf) {
        self.paths.insert(path, Instant::now());
    }

    fn should_ignore(&mut self, path: &Path) -> bool {
        self.prune();
        self.paths.contains_key(path)
    }

    fn prune(&mut self) {
        let now = Instant::now();
        self.paths
            .retain(|_, instant| now.duration_since(*instant) < self.ignore_duration);
    }
}

#[cfg(test)]
mod tests {
    use std::fs;

    use tempfile::tempdir;

    use super::*;

    #[test]
    fn pending_events_return_only_after_debounce() {
        let mut pending = PendingEvents::new(Duration::from_millis(20));
        pending.insert(PathBuf::from("/tmp/a"));

        assert!(pending.ready_paths().is_empty());
        thread::sleep(Duration::from_millis(30));
        assert_eq!(pending.ready_paths(), vec![PathBuf::from("/tmp/a")]);
        assert!(pending.ready_paths().is_empty());
    }

    #[test]
    fn pending_events_reset_timer_for_same_path() {
        let mut pending = PendingEvents::new(Duration::from_millis(40));
        pending.insert(PathBuf::from("/tmp/a"));
        thread::sleep(Duration::from_millis(25));
        pending.insert(PathBuf::from("/tmp/a"));
        thread::sleep(Duration::from_millis(25));

        assert!(pending.ready_paths().is_empty());
        thread::sleep(Duration::from_millis(20));
        assert_eq!(pending.ready_paths(), vec![PathBuf::from("/tmp/a")]);
    }

    #[test]
    fn recent_conversions_ignore_temporarily() {
        let mut recent = RecentConversions::new(Duration::from_millis(20));
        let path = PathBuf::from("/tmp/a");

        recent.mark(path.clone());
        assert!(recent.should_ignore(&path));
        thread::sleep(Duration::from_millis(30));
        assert!(!recent.should_ignore(&path));
    }

    #[test]
    fn sweep_interval_grows_with_sweep_cost() {
        assert_eq!(next_sweep_interval(Duration::from_millis(30)), SWEEP_INTERVAL);
        assert_eq!(next_sweep_interval(Duration::from_millis(300)), Duration::from_secs(15));
        assert_eq!(next_sweep_interval(Duration::from_secs(5)), MAX_SWEEP_INTERVAL);
    }

    fn rename_result(status: &str, reason: Option<&str>) -> RenameResult {
        RenameResult {
            status: status.to_string(),
            kind: "file".to_string(),
            original_path: PathBuf::from("/tmp/a"),
            final_path: None,
            original_name: "a".to_string(),
            final_name: "a".to_string(),
            collision: false,
            reason: reason.map(ToString::to_string),
        }
    }

    #[test]
    fn repeated_failures_are_reported_once() {
        let mut reported = ReportedFailures::default();
        let busy = rename_result("failed", Some("busy"));

        assert!(reported.should_report(&busy));
        assert!(!reported.should_report(&busy));
        assert!(reported.should_report(&rename_result("failed", Some("other"))));
        assert!(reported.should_report(&rename_result("success", None)));
        assert!(reported.should_report(&busy));
    }

    #[test]
    fn watch_service_normalizes_created_file() {
        let directory = tempdir().unwrap();
        let log_file_path = directory.path().join("activity.jsonl");
        let watch_path = directory.path().join("watch");
        fs::create_dir(&watch_path).unwrap();

        let mut service = WatchService::new();
        let status = service
            .start(
                AppSettings {
                    watch_folders: vec![WatchFolder {
                        path: watch_path.clone(),
                        recursive: true,
                        enabled: true,
                    }],
                    auto_watch_enabled: true,
                    launch_at_login: false,
                    stabilization_wait_ms: 1_000,
                    default_recursive: true,
                },
                log_file_path.clone(),
                None,
            )
            .unwrap();

        assert!(status.running);

        let original_path = watch_path.join("자동감시.txt");
        let final_path = watch_path.join("자동감시.txt");
        fs::write(&original_path, "auto watch test").unwrap();

        let deadline = Instant::now() + Duration::from_secs(12);
        while Instant::now() < deadline {
            if final_path.exists() && log_file_path.exists() {
                break;
            }
            thread::sleep(Duration::from_millis(100));
        }

        service.stop();

        assert!(final_path.exists());
        assert!(log_file_path.exists());
    }

    #[test]
    fn watch_service_does_not_suffix_after_first_success() {
        let directory = tempdir().unwrap();
        let log_file_path = directory.path().join("activity.jsonl");
        let watch_path = directory.path().join("watch");
        fs::create_dir(&watch_path).unwrap();

        let mut service = WatchService::new();
        service
            .start(
                AppSettings {
                    watch_folders: vec![WatchFolder {
                        path: watch_path.clone(),
                        recursive: true,
                        enabled: true,
                    }],
                    auto_watch_enabled: true,
                    launch_at_login: false,
                    stabilization_wait_ms: 1_000,
                    default_recursive: true,
                },
                log_file_path.clone(),
                None,
            )
            .unwrap();

        let original_path = watch_path.join("성경 말씀기.md");
        let final_path = watch_path.join("성경 말씀기.md");
        let suffixed_path = watch_path.join("성경 말씀기_1.md");
        fs::write(&original_path, "auto watch test").unwrap();

        let deadline = Instant::now() + Duration::from_secs(12);
        while Instant::now() < deadline {
            if final_path.exists() && log_file_path.exists() {
                break;
            }
            thread::sleep(Duration::from_millis(100));
        }

        thread::sleep(SWEEP_INTERVAL + Duration::from_secs(1));
        service.stop();

        assert!(final_path.exists());
        assert!(!suffixed_path.exists());

        let log_content = fs::read_to_string(log_file_path).unwrap();
        assert!(!log_content.contains("성경 말씀기_1.md"));
    }
}
