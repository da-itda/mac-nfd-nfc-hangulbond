use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;

use serde::Serialize;

use crate::normalizer::{
    build_collision_safe_name, create_rename_plan, is_temporary_or_ignored_name,
};
use crate::stabilization::{wait_for_stable_paths, StabilityOptions, StabilityStatus};

const EXCLUDED_DIRECTORY_NAMES: &[&str] = &[".git", "node_modules"];

#[derive(Debug, Clone, Serialize, serde::Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ScanTarget {
    pub kind: String,
    pub path: PathBuf,
    pub parent_path: PathBuf,
    pub original_name: String,
    pub normalized_name: String,
    pub final_name: String,
    pub collision: bool,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct RenameResult {
    pub status: String,
    pub kind: String,
    pub original_path: PathBuf,
    pub final_path: Option<PathBuf>,
    pub original_name: String,
    pub final_name: String,
    pub collision: bool,
    pub reason: Option<String>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct NormalizeProgress {
    pub phase: String,
    pub completed: usize,
    pub total: usize,
    pub current_name: Option<String>,
    pub final_name: Option<String>,
    pub status: Option<String>,
    pub reason: Option<String>,
}

#[derive(Debug, Clone, Copy)]
pub struct NormalizeOptions {
    pub recursive: bool,
    pub dry_run: bool,
    pub stabilization_wait_ms: u64,
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ScanOutcome {
    pub targets: Vec<ScanTarget>,
    pub cancelled: bool,
    pub scanned_entries: usize,
}

#[derive(Debug, Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct NormalizeOutcome {
    pub results: Vec<RenameResult>,
    pub cancelled: bool,
}

pub fn scan_directory(root_path: &Path, recursive: bool) -> Result<Vec<ScanTarget>, String> {
    let cancel = AtomicBool::new(false);
    Ok(scan_directory_interruptible(root_path, recursive, &cancel, |_| {})?.targets)
}

pub fn scan_directory_interruptible<F>(
    root_path: &Path,
    recursive: bool,
    cancel: &AtomicBool,
    mut on_progress: F,
) -> Result<ScanOutcome, String>
where
    F: FnMut(usize),
{
    let mut targets = Vec::new();
    let mut scanned_entries = 0;
    walk_directory(
        root_path,
        recursive,
        &mut targets,
        cancel,
        &mut scanned_entries,
        &mut on_progress,
    )?;
    Ok(ScanOutcome {
        targets,
        cancelled: cancel.load(Ordering::Relaxed),
        scanned_entries,
    })
}

pub fn normalize_selected_targets<F>(
    root_path: &Path,
    recursive: bool,
    selected_paths: &[PathBuf],
    options: NormalizeOptions,
    cancel: &AtomicBool,
    mut on_progress: F,
) -> Result<NormalizeOutcome, String>
where
    F: FnMut(NormalizeProgress),
{
    let selected = selected_paths.iter().cloned().collect::<HashSet<_>>();
    let scan = scan_directory_interruptible(root_path, recursive, cancel, |scanned| {
        on_progress(NormalizeProgress {
            phase: "preparing".to_string(),
            completed: scanned,
            total: 0,
            current_name: None,
            final_name: None,
            status: None,
            reason: None,
        });
    })?;
    let targets = scan
        .targets
        .into_iter()
        .filter(|target| selected.contains(&target.path))
        .collect::<Vec<_>>();
    let total = targets.len();
    let mut results = Vec::with_capacity(total);

    on_progress(NormalizeProgress {
        phase: "started".to_string(),
        completed: 0,
        total,
        current_name: None,
        final_name: None,
        status: None,
        reason: None,
    });

    let statuses = stability_for_targets(&targets, options.stabilization_wait_ms);
    for (target, status) in targets.into_iter().zip(statuses) {
        if cancel.load(Ordering::Relaxed) {
            break;
        }

        let result = rename_target(&target, status);
        let completed = results.len() + 1;
        on_progress(NormalizeProgress {
            phase: "item".to_string(),
            completed,
            total,
            current_name: Some(result.original_name.clone()),
            final_name: Some(result.final_name.clone()),
            status: Some(result.status.clone()),
            reason: result.reason.clone(),
        });
        results.push(result);
        thread::yield_now();
    }

    let cancelled = cancel.load(Ordering::Relaxed);
    on_progress(NormalizeProgress {
        phase: if cancelled { "cancelled" } else { "completed" }.to_string(),
        completed: results.len(),
        total,
        current_name: None,
        final_name: None,
        status: None,
        reason: None,
    });

    Ok(NormalizeOutcome { results, cancelled })
}

#[cfg(test)]
pub fn normalize_path(path: &Path, options: NormalizeOptions) -> Result<Vec<RenameResult>, String> {
    normalize_paths(&[path.to_path_buf()], options)
}

// 감시 중 한꺼번에 생긴 파일들을 처리한다. 폴더는 안쪽까지 정리하고, 파일은 안정 대기를 한 번만 한다.
pub fn normalize_paths(paths: &[PathBuf], options: NormalizeOptions) -> Result<Vec<RenameResult>, String> {
    let mut results = Vec::new();
    let mut targets = Vec::new();
    let mut listings = HashMap::new();

    for path in paths {
        let planned = fs::metadata(path)
            .map_err(|error| format!("failed to inspect {}: {error}", path.display()))
            .and_then(|metadata| {
                if metadata.is_dir() {
                    results.extend(normalize_directory(path, options)?);
                    Ok(None)
                } else {
                    plan_file_target(path, &metadata, &mut listings)
                }
            });
        match planned {
            Ok(Some(target)) => targets.push(target),
            Ok(None) => {}
            Err(error) if paths.len() == 1 => return Err(error),
            // 여러 경로 중 하나가 그새 사라져도 나머지는 계속 처리한다.
            Err(error) => eprintln!("{error}"),
        }
    }

    if options.dry_run {
        results.extend(targets.iter().map(dry_run_result));
    } else {
        let statuses = stability_for_targets(&targets, options.stabilization_wait_ms);
        results.extend(
            targets
                .iter()
                .zip(statuses)
                .map(|(target, status)| rename_target(target, status)),
        );
    }

    Ok(results)
}

// 감시로 들어온 파일들의 부모 폴더 목록. 폴더마다 한 번만 읽어서, 큰 폴더에 파일이
// 한꺼번에 들어와도 파일 수만큼 폴더 전체를 다시 읽지 않는다.
struct ParentListing {
    names: HashSet<String>,
    identities: HashMap<EntryIdentity, (PathBuf, String)>,
}

fn read_parent_listing(parent_path: &Path) -> Result<ParentListing, String> {
    let mut listing = ParentListing {
        names: HashSet::new(),
        identities: HashMap::new(),
    };
    for entry in fs::read_dir(parent_path)
        .map_err(|error| format!("failed to read {}: {error}", parent_path.display()))?
    {
        let entry = entry.map_err(|error| {
            format!("failed to read entry in {}: {error}", parent_path.display())
        })?;
        let Some(name) = entry.file_name().to_str().map(ToString::to_string) else {
            continue;
        };
        if let Some(identity) = dir_entry_identity(&entry) {
            listing.identities.insert(identity, (entry.path(), name.clone()));
        }
        listing.names.insert(name);
    }
    Ok(listing)
}

fn plan_file_target(
    path: &Path,
    metadata: &fs::Metadata,
    listings: &mut HashMap<PathBuf, ParentListing>,
) -> Result<Option<ScanTarget>, String> {
    let parent_path = path
        .parent()
        .ok_or_else(|| format!("failed to resolve parent for {}", path.display()))?
        .to_path_buf();
    let listing = match listings.entry(parent_path.clone()) {
        std::collections::hash_map::Entry::Occupied(entry) => entry.into_mut(),
        std::collections::hash_map::Entry::Vacant(entry) => {
            entry.insert(read_parent_listing(&parent_path)?)
        }
    };

    // 이벤트 경로와 실제 이름의 정규화 형태가 다를 수 있어(macOS), 같은 파일을 찾아 실제 이름을 쓴다.
    let (current_path, file_name) = metadata_identity(metadata)
        .and_then(|identity| listing.identities.get(&identity).cloned())
        .unwrap_or_else(|| {
            let fallback_name = path
                .file_name()
                .and_then(|value| value.to_str())
                .unwrap_or_default()
                .to_string();
            (path.to_path_buf(), fallback_name)
        });

    if file_name.is_empty() {
        return Err(format!("failed to read file name for {}", path.display()));
    }

    let plan = create_rename_plan(&file_name, &listing.names);

    if !plan.analysis.should_rename {
        return Ok(None);
    }
    listing.names.insert(plan.final_name.clone());

    Ok(Some(ScanTarget {
        kind: "file".to_string(),
        path: current_path,
        parent_path,
        original_name: plan.analysis.original_name,
        normalized_name: plan.analysis.normalized_name,
        final_name: plan.final_name,
        collision: plan.collision,
    }))
}

fn dry_run_result(target: &ScanTarget) -> RenameResult {
    RenameResult {
        status: "dry-run".to_string(),
        kind: target.kind.clone(),
        original_path: target.path.clone(),
        final_path: Some(target.parent_path.join(&target.final_name)),
        original_name: target.original_name.clone(),
        final_name: target.final_name.clone(),
        collision: target.collision,
        reason: None,
    }
}

pub fn normalize_directory(
    root_path: &Path,
    options: NormalizeOptions,
) -> Result<Vec<RenameResult>, String> {
    normalize_directory_with_progress(root_path, options, |_| {})
}

pub fn normalize_directory_with_progress<F>(
    root_path: &Path,
    options: NormalizeOptions,
    mut on_progress: F,
) -> Result<Vec<RenameResult>, String>
where
    F: FnMut(NormalizeProgress),
{
    let targets = scan_directory(root_path, options.recursive)?;
    let total = targets.len();
    let mut results = Vec::with_capacity(targets.len());

    on_progress(NormalizeProgress {
        phase: "started".to_string(),
        completed: 0,
        total,
        current_name: None,
        final_name: None,
        status: None,
        reason: None,
    });

    let statuses = if options.dry_run {
        vec![StabilityStatus::Stable; targets.len()]
    } else {
        stability_for_targets(&targets, options.stabilization_wait_ms)
    };
    for (target, status) in targets.into_iter().zip(statuses) {
        let result = if options.dry_run {
            dry_run_result(&target)
        } else {
            rename_target(&target, status)
        };

        let completed = results.len() + 1;
        on_progress(NormalizeProgress {
            phase: "item".to_string(),
            completed,
            total,
            current_name: Some(result.original_name.clone()),
            final_name: Some(result.final_name.clone()),
            status: Some(result.status.clone()),
            reason: result.reason.clone(),
        });
        results.push(result);
    }

    on_progress(NormalizeProgress {
        phase: "completed".to_string(),
        completed: total,
        total,
        current_name: None,
        final_name: None,
        status: None,
        reason: None,
    });

    Ok(results)
}

fn walk_directory(
    directory_path: &Path,
    recursive: bool,
    targets: &mut Vec<ScanTarget>,
    cancel: &AtomicBool,
    scanned_entries: &mut usize,
    on_progress: &mut impl FnMut(usize),
) -> Result<(), String> {
    let read_dir = fs::read_dir(directory_path)
        .map_err(|error| format!("failed to read {}: {error}", directory_path.display()))?;
    let mut entries = Vec::new();
    for entry in read_dir {
        if cancel.load(Ordering::Relaxed) {
            return Ok(());
        }
        let entry = entry.map_err(|error| {
            format!(
                "failed to read entry in {}: {error}",
                directory_path.display()
            )
        })?;
        *scanned_entries += 1;
        if *scanned_entries % 64 == 0 {
            on_progress(*scanned_entries);
            thread::yield_now();
        }
        entries.push(entry);
    }

    let mut reserved_names: HashSet<String> = entries
        .iter()
        .filter_map(|entry| entry.file_name().to_str().map(ToString::to_string))
        .collect();

    for entry in entries {
        if cancel.load(Ordering::Relaxed) {
            return Ok(());
        }

        let file_type = entry
            .file_type()
            .map_err(|error| format!("failed to inspect {}: {error}", entry.path().display()))?;

        if file_type.is_symlink() {
            continue;
        }

        let file_name = match entry.file_name().to_str() {
            Some(value) => value.to_string(),
            None => continue,
        };

        if file_type.is_dir() && recursive && !is_excluded_directory_name(&file_name) {
            walk_directory(
                &entry.path(),
                recursive,
                targets,
                cancel,
                scanned_entries,
                on_progress,
            )?;
        }

        if !file_type.is_file() && !file_type.is_dir() {
            continue;
        }

        let plan = create_rename_plan(&file_name, &reserved_names);

        if !plan.analysis.should_rename {
            continue;
        }

        reserved_names.insert(plan.final_name.clone());
        targets.push(ScanTarget {
            kind: if file_type.is_dir() {
                "directory"
            } else {
                "file"
            }
            .to_string(),
            path: entry.path(),
            parent_path: directory_path.to_path_buf(),
            original_name: plan.analysis.original_name,
            normalized_name: plan.analysis.normalized_name,
            final_name: plan.final_name,
            collision: plan.collision,
        });
    }

    Ok(())
}

// 파일은 쓰기가 끝났는지 한꺼번에 확인한다. 폴더는 바로 바꿀 수 있다.
fn stability_for_targets(targets: &[ScanTarget], stabilization_wait_ms: u64) -> Vec<StabilityStatus> {
    let file_paths = targets
        .iter()
        .filter(|target| target.kind == "file")
        .map(|target| target.path.clone())
        .collect::<Vec<_>>();
    let mut file_statuses = wait_for_stable_paths(
        &file_paths,
        StabilityOptions::from_millis(stabilization_wait_ms),
    )
    .into_iter();
    targets
        .iter()
        .map(|target| {
            if target.kind == "file" {
                file_statuses.next().unwrap_or(StabilityStatus::Missing)
            } else {
                StabilityStatus::Stable
            }
        })
        .collect()
}

fn rename_target(target: &ScanTarget, status: StabilityStatus) -> RenameResult {
    if target.kind == "file" {
        if status != StabilityStatus::Stable {
            return RenameResult {
                status: "failed".to_string(),
                kind: target.kind.clone(),
                original_path: target.path.clone(),
                final_path: None,
                original_name: target.original_name.clone(),
                final_name: target.final_name.clone(),
                collision: target.collision,
                reason: Some(format!("{status:?}")),
            };
        }
    }

    if let Some(result) = remove_identical_duplicate(target) {
        return result;
    }

    let refreshed = refresh_collision_target(target);
    let final_path = refreshed.parent_path.join(&refreshed.final_name);

    match fs::rename(&refreshed.path, &final_path) {
        Ok(()) => RenameResult {
            status: if refreshed.collision {
                "collision".to_string()
            } else {
                "success".to_string()
            },
            kind: refreshed.kind,
            original_path: refreshed.path,
            final_path: Some(final_path),
            original_name: refreshed.original_name,
            final_name: refreshed.final_name,
            collision: refreshed.collision,
            reason: None,
        },
        Err(error) => RenameResult {
            status: "failed".to_string(),
            kind: refreshed.kind,
            original_path: refreshed.path,
            final_path: None,
            original_name: refreshed.original_name,
            final_name: refreshed.final_name,
            collision: refreshed.collision,
            reason: Some(rename_error_reason(&error)),
        },
    }
}

// 정리한 뒤 다른 앱이 같은 파일을 원래(NFD) 이름으로 다시 저장하는 경우가 있다(Windows 텔레그램).
// NFC 이름의 파일이 이미 있고 내용까지 같으면 `_1` 사본을 만들지 않고 NFD 쪽을 지운다.
// 내용이 하나라도 다르거나 비교할 수 없으면 None을 돌려 기존처럼 `_1`을 붙인다.
fn remove_identical_duplicate(target: &ScanTarget) -> Option<RenameResult> {
    if target.kind != "file" {
        return None;
    }
    let existing_path = target.parent_path.join(&target.normalized_name);
    if !existing_path.is_file()
        || paths_refer_to_same_entry(&target.path, &existing_path)
        || !files_have_same_content(&target.path, &existing_path)
    {
        return None;
    }

    let (status, final_path, reason) = match fs::remove_file(&target.path) {
        Ok(()) => ("duplicate", Some(existing_path), None),
        Err(error) => ("failed", None, Some(rename_error_reason(&error))),
    };
    Some(RenameResult {
        status: status.to_string(),
        kind: target.kind.clone(),
        original_path: target.path.clone(),
        final_path,
        original_name: target.original_name.clone(),
        final_name: target.normalized_name.clone(),
        collision: false,
        reason,
    })
}

// 크기를 먼저 보고, 같으면 끝까지 바이트를 비교한다. 아주 큰 파일은 비교하지 않는다(감시가 오래 멈추지 않게).
const MAX_DUPLICATE_COMPARE_BYTES: u64 = 1024 * 1024 * 1024;

fn files_have_same_content(left: &Path, right: &Path) -> bool {
    use std::io::Read;

    let (Ok(left_meta), Ok(right_meta)) = (fs::metadata(left), fs::metadata(right)) else {
        return false;
    };
    if left_meta.len() != right_meta.len() || left_meta.len() > MAX_DUPLICATE_COMPARE_BYTES {
        return false;
    }
    let (Ok(mut left_file), Ok(mut right_file)) = (fs::File::open(left), fs::File::open(right)) else {
        return false;
    };
    let mut left_buffer = vec![0u8; 64 * 1024];
    let mut right_buffer = vec![0u8; 64 * 1024];
    loop {
        let Ok(read) = left_file.read(&mut left_buffer) else {
            return false;
        };
        if read == 0 {
            // 왼쪽이 끝났으면 오른쪽도 끝나야 같다.
            return matches!(right_file.read(&mut right_buffer[..1]), Ok(0));
        }
        if right_file.read_exact(&mut right_buffer[..read]).is_err()
            || left_buffer[..read] != right_buffer[..read]
        {
            return false;
        }
    }
}

// Windows는 다른 프로그램이 연 파일의 이름을 바꿀 수 없다. 주기 검사에서 다시 시도하므로 그 사실을 알린다.
fn rename_error_reason(error: &std::io::Error) -> String {
    #[cfg(windows)]
    {
        const ERROR_SHARING_VIOLATION: i32 = 32;
        const ERROR_LOCK_VIOLATION: i32 = 33;
        if matches!(
            error.raw_os_error(),
            Some(ERROR_SHARING_VIOLATION | ERROR_LOCK_VIOLATION)
        ) {
            return "다른 프로그램이 파일을 사용 중입니다. 닫으면 다시 시도합니다.".to_string();
        }
    }
    error.to_string()
}

fn refresh_collision_target(target: &ScanTarget) -> ScanTarget {
    let target_path = target.parent_path.join(&target.final_name);
    // 폴더 목록은 실제로 이름이 겹칠 때만 읽는다. 큰 폴더에서 파일마다 다시 읽지 않기 위해서다.
    let final_name =
        if target_path.exists() && !paths_refer_to_same_entry(&target.path, &target_path) {
            let current_names = fs::read_dir(&target.parent_path)
                .ok()
                .into_iter()
                .flat_map(|entries| entries.filter_map(Result::ok))
                .filter_map(|entry| entry.file_name().to_str().map(ToString::to_string))
                .collect::<HashSet<_>>();
            build_collision_safe_name(&target.normalized_name, &current_names)
        } else {
            target.final_name.clone()
        };

    ScanTarget {
        final_name: final_name.clone(),
        collision: target.collision || final_name != target.normalized_name,
        ..target.clone()
    }
}

// 같은 파일인지 가리는 식별값(장치·inode). Windows에서는 쓰지 않으므로 폴더 항목마다 메타데이터를 읽지 않는다.
type EntryIdentity = (u64, u64);

#[cfg(unix)]
fn metadata_identity(metadata: &fs::Metadata) -> Option<EntryIdentity> {
    use std::os::unix::fs::MetadataExt;

    Some((metadata.dev(), metadata.ino()))
}

#[cfg(not(unix))]
fn metadata_identity(_metadata: &fs::Metadata) -> Option<EntryIdentity> {
    None
}

#[cfg(unix)]
fn dir_entry_identity(entry: &fs::DirEntry) -> Option<EntryIdentity> {
    entry.metadata().ok().as_ref().and_then(metadata_identity)
}

#[cfg(not(unix))]
fn dir_entry_identity(_entry: &fs::DirEntry) -> Option<EntryIdentity> {
    None
}

#[cfg(unix)]
fn paths_refer_to_same_entry(left: &Path, right: &Path) -> bool {
    match (fs::metadata(left), fs::metadata(right)) {
        (Ok(left_metadata), Ok(right_metadata)) => {
            metadata_identity(&left_metadata) == metadata_identity(&right_metadata)
        }
        _ => false,
    }
}

#[cfg(not(unix))]
fn paths_refer_to_same_entry(left: &Path, right: &Path) -> bool {
    match (fs::canonicalize(left), fs::canonicalize(right)) {
        (Ok(left_path), Ok(right_path)) => left_path == right_path,
        _ => false,
    }
}

fn is_excluded_directory_name(name: &str) -> bool {
    EXCLUDED_DIRECTORY_NAMES.contains(&name)
        || name.ends_with(".app")
        || is_temporary_or_ignored_name(name)
}

#[cfg(test)]
mod tests {
    use std::io::ErrorKind;

    use tempfile::tempdir;
    use unicode_normalization::UnicodeNormalization;

    use super::*;

    #[test]
    fn scans_nfd_file_names() {
        let directory = tempdir().unwrap();
        fs::write(directory.path().join("한글.txt"), "content").unwrap();

        let targets = scan_directory(directory.path(), true).unwrap();

        assert_eq!(targets.len(), 1);
        assert_eq!(targets[0].original_name, "한글.txt");
        assert_eq!(targets[0].final_name, "한글.txt");
    }

    #[test]
    fn renames_nfd_file_and_preserves_content() {
        let directory = tempdir().unwrap();
        let original_path = directory.path().join("한글.txt");
        let final_path = directory.path().join("한글.txt");
        fs::write(&original_path, "파일 내용은 바꾸지 않습니다.").unwrap();

        let results = normalize_directory(
            directory.path(),
            NormalizeOptions {
                recursive: true,
                dry_run: false,
                stabilization_wait_ms: 0,
            },
        )
        .unwrap();

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].status, "success");
        assert_eq!(
            fs::read_to_string(final_path).unwrap(),
            "파일 내용은 바꾸지 않습니다."
        );
    }

    #[test]
    fn does_not_overwrite_existing_nfc_file_when_filesystem_supports_collision_case() {
        let directory = tempdir().unwrap();

        if !supports_distinct_normalization_names(directory.path()) {
            return;
        }

        fs::write(directory.path().join("가.txt"), "original").unwrap();
        fs::write(directory.path().join("가.txt"), "nfd").unwrap();

        let results = normalize_directory(
            directory.path(),
            NormalizeOptions {
                recursive: true,
                dry_run: false,
                stabilization_wait_ms: 0,
            },
        )
        .unwrap();

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].status, "collision");
        assert_eq!(results[0].final_name, "가_1.txt");
        assert_eq!(
            fs::read_to_string(directory.path().join("가.txt")).unwrap(),
            "original"
        );
        assert_eq!(
            fs::read_to_string(directory.path().join("가_1.txt")).unwrap(),
            "nfd"
        );
    }

    #[test]
    fn removes_nfd_copy_when_identical_nfc_file_exists() {
        let directory = tempdir().unwrap();

        if !supports_distinct_normalization_names(directory.path()) {
            return;
        }

        let nfd_name = "가.txt".nfd().collect::<String>();
        fs::write(directory.path().join("가.txt"), "same").unwrap();
        fs::write(directory.path().join(&nfd_name), "same").unwrap();

        let results = normalize_directory(
            directory.path(),
            NormalizeOptions {
                recursive: true,
                dry_run: false,
                stabilization_wait_ms: 0,
            },
        )
        .unwrap();

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].status, "duplicate");
        assert_eq!(results[0].final_name, "가.txt");
        assert!(!directory.path().join(&nfd_name).exists());
        assert!(!directory.path().join("가_1.txt").exists());
        assert_eq!(
            fs::read_to_string(directory.path().join("가.txt")).unwrap(),
            "same"
        );
    }

    #[test]
    fn compares_file_contents_exactly() {
        let directory = tempdir().unwrap();
        let a = directory.path().join("a");
        let b = directory.path().join("b");
        let c = directory.path().join("c");
        let d = directory.path().join("d");
        let big = vec![7u8; 200 * 1024];
        let mut changed = big.clone();
        changed[150 * 1024] = 8;
        fs::write(&a, &big).unwrap();
        fs::write(&b, &big).unwrap();
        fs::write(&c, &changed).unwrap();
        fs::write(&d, &big[..1000]).unwrap();

        assert!(files_have_same_content(&a, &b));
        assert!(!files_have_same_content(&a, &c));
        assert!(!files_have_same_content(&a, &d));
        assert!(!files_have_same_content(&a, &directory.path().join("missing")));
    }

    #[test]
    fn renames_child_entries_before_parent_directories() {
        let directory = tempdir().unwrap();
        let nfd_directory = directory.path().join("한글폴더");
        fs::create_dir(&nfd_directory).unwrap();
        fs::write(nfd_directory.join("가.txt"), "nested").unwrap();

        let results = normalize_directory(
            directory.path(),
            NormalizeOptions {
                recursive: true,
                dry_run: false,
                stabilization_wait_ms: 0,
            },
        )
        .unwrap();

        assert_eq!(results.len(), 2);
        assert_eq!(results[0].kind, "file");
        assert_eq!(results[1].kind, "directory");
        assert_eq!(
            fs::read_to_string(directory.path().join("한글폴더").join("가.txt")).unwrap(),
            "nested"
        );
    }

    #[test]
    fn ignores_temporary_files() {
        let directory = tempdir().unwrap();
        let ignored_path = directory.path().join("보고서.tmp");
        fs::write(&ignored_path, "temporary").unwrap();

        let results = normalize_directory(
            directory.path(),
            NormalizeOptions {
                recursive: true,
                dry_run: false,
                stabilization_wait_ms: 0,
            },
        )
        .unwrap();

        assert!(results.is_empty());
        assert_eq!(fs::read_to_string(ignored_path).unwrap(), "temporary");
    }

    #[test]
    fn normalizes_single_path() {
        let directory = tempdir().unwrap();
        let original_path = directory.path().join("한글.txt");
        fs::write(&original_path, "content").unwrap();

        let results = normalize_path(
            &original_path,
            NormalizeOptions {
                recursive: true,
                dry_run: false,
                stabilization_wait_ms: 0,
            },
        )
        .unwrap();

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].final_name, "한글.txt");
        assert_eq!(
            fs::read_to_string(directory.path().join("한글.txt")).unwrap(),
            "content"
        );
    }

    #[test]
    fn ignores_stale_nfd_lookup_when_entry_is_already_nfc() {
        let directory = tempdir().unwrap();
        let nfc_name = "풍력발전 사업계획서(중국).pptx";
        let nfd_name = nfc_name.nfd().collect::<String>();
        let nfc_path = directory.path().join(nfc_name);
        let stale_nfd_path = directory.path().join(&nfd_name);
        let suffixed_path = directory.path().join("풍력발전 사업계획서(중국)_1.pptx");

        fs::write(&nfc_path, "presentation").unwrap();

        if fs::metadata(&stale_nfd_path).is_err() {
            return;
        }

        let results = normalize_path(
            &stale_nfd_path,
            NormalizeOptions {
                recursive: true,
                dry_run: false,
                stabilization_wait_ms: 0,
            },
        )
        .unwrap();

        assert!(results.is_empty());
        assert!(nfc_path.exists());
        assert!(!suffixed_path.exists());
    }

    fn supports_distinct_normalization_names(root: &Path) -> bool {
        let probe = root.join("collision-probe");
        fs::create_dir(&probe).unwrap();
        fs::write(probe.join("가.txt"), "nfc").unwrap();
        match fs::write(probe.join("가.txt"), "nfd") {
            Ok(()) => {}
            Err(error) if error.kind() == ErrorKind::AlreadyExists => return false,
            Err(error) => panic!("failed to create probe file: {error}"),
        }

        let distinct = fs::read_dir(&probe).unwrap().count() == 2;
        fs::remove_dir_all(&probe).unwrap();
        distinct
    }
}
