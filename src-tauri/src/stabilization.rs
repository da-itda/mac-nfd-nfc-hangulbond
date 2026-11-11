use std::fs;
use std::path::PathBuf;
use std::thread;
use std::time::{Duration, SystemTime};

use crate::normalizer::is_temporary_or_ignored_name;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StabilityStatus {
    Stable,
    Missing,
    IgnoredTemporary,
    Changing,
    Inaccessible(String),
}

#[derive(Debug, Clone, Copy)]
pub struct StabilityOptions {
    pub initial_wait: Duration,
    pub sample_wait: Duration,
}

impl StabilityOptions {
    pub fn from_millis(initial_wait_ms: u64) -> Self {
        Self {
            initial_wait: Duration::from_millis(initial_wait_ms),
            sample_wait: Duration::from_millis(250),
        }
    }
}

#[cfg(test)]
pub fn wait_for_stable_path(path: &std::path::Path, options: StabilityOptions) -> StabilityStatus {
    wait_for_stable_paths(&[path.to_path_buf()], options)
        .pop()
        .unwrap_or(StabilityStatus::Missing)
}

// 여러 파일을 한꺼번에 기다린다. 대기와 표본 간격을 파일마다가 아니라 한 번만 적용해서,
// 압축을 풀어 생긴 파일 수백 개도 몇 초 안에 확인한다.
pub fn wait_for_stable_paths(paths: &[PathBuf], options: StabilityOptions) -> Vec<StabilityStatus> {
    let mut statuses = paths
        .iter()
        .map(|path| {
            let temporary = path
                .file_name()
                .and_then(|value| value.to_str())
                .is_some_and(is_temporary_or_ignored_name);
            temporary.then_some(StabilityStatus::IgnoredTemporary)
        })
        .collect::<Vec<_>>();

    if statuses.iter().all(Option::is_some) {
        return statuses.into_iter().flatten().collect();
    }

    if options.initial_wait > Duration::ZERO {
        thread::sleep(options.initial_wait);
    }

    let mut first = vec![None; paths.len()];
    for (index, path) in paths.iter().enumerate() {
        if statuses[index].is_some() {
            continue;
        }
        match fs::metadata(path) {
            Ok(metadata) if metadata.is_dir() => statuses[index] = Some(StabilityStatus::Stable),
            Ok(metadata) => first[index] = Some(metadata),
            Err(error) => statuses[index] = Some(metadata_error_status(error)),
        }
    }

    if first.iter().any(Option::is_some) {
        thread::sleep(options.sample_wait);
    }

    for (index, path) in paths.iter().enumerate() {
        let Some(first) = &first[index] else {
            continue;
        };
        statuses[index] = Some(match fs::metadata(path) {
            Ok(second)
                if first.len() == second.len()
                    && modified_time(first) == modified_time(&second) =>
            {
                StabilityStatus::Stable
            }
            Ok(_) => StabilityStatus::Changing,
            Err(error) => metadata_error_status(error),
        });
    }

    statuses
        .into_iter()
        .map(|status| status.unwrap_or(StabilityStatus::Missing))
        .collect()
}

fn metadata_error_status(error: std::io::Error) -> StabilityStatus {
    if error.kind() == std::io::ErrorKind::NotFound {
        StabilityStatus::Missing
    } else {
        StabilityStatus::Inaccessible(error.to_string())
    }
}

fn modified_time(metadata: &fs::Metadata) -> Option<SystemTime> {
    metadata.modified().ok()
}

#[cfg(test)]
mod tests {
    use std::fs::{self, OpenOptions};
    use std::io::Write;

    use tempfile::tempdir;

    use super::*;

    #[test]
    fn reports_stable_file_after_sample() {
        let directory = tempdir().unwrap();
        let path = directory.path().join("한글.txt");
        fs::write(&path, "content").unwrap();

        let status = wait_for_stable_path(
            &path,
            StabilityOptions {
                initial_wait: Duration::ZERO,
                sample_wait: Duration::from_millis(10),
            },
        );

        assert_eq!(status, StabilityStatus::Stable);
    }

    #[test]
    fn reports_missing_file() {
        let directory = tempdir().unwrap();
        let path = directory.path().join("missing.txt");

        let status = wait_for_stable_path(
            &path,
            StabilityOptions {
                initial_wait: Duration::ZERO,
                sample_wait: Duration::from_millis(10),
            },
        );

        assert_eq!(status, StabilityStatus::Missing);
    }

    #[test]
    fn reports_temporary_file_without_waiting() {
        let directory = tempdir().unwrap();
        let path = directory.path().join("보고서.tmp");
        fs::write(&path, "content").unwrap();

        let status = wait_for_stable_path(
            &path,
            StabilityOptions {
                initial_wait: Duration::from_secs(1),
                sample_wait: Duration::from_millis(10),
            },
        );

        assert_eq!(status, StabilityStatus::IgnoredTemporary);
    }

    #[test]
    fn waits_once_for_many_files() {
        let directory = tempdir().unwrap();
        let paths = (0..50)
            .map(|index| {
                let path = directory.path().join(format!("파일{index}.txt"));
                fs::write(&path, "content").unwrap();
                path
            })
            .chain([directory.path().join("missing.txt")])
            .collect::<Vec<_>>();

        let started = std::time::Instant::now();
        let statuses = wait_for_stable_paths(
            &paths,
            StabilityOptions {
                initial_wait: Duration::from_millis(100),
                sample_wait: Duration::from_millis(50),
            },
        );

        assert!(started.elapsed() < Duration::from_secs(1));
        assert!(statuses[..50].iter().all(|status| *status == StabilityStatus::Stable));
        assert_eq!(statuses[50], StabilityStatus::Missing);
    }

    #[test]
    fn reports_changing_file() {
        let directory = tempdir().unwrap();
        let path = directory.path().join("한글.txt");
        fs::write(&path, "content").unwrap();
        let changing_path = path.clone();

        let handle = std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(20));
            let mut file = OpenOptions::new().append(true).open(changing_path).unwrap();
            writeln!(file, "more").unwrap();
        });

        let status = wait_for_stable_path(
            &path,
            StabilityOptions {
                initial_wait: Duration::ZERO,
                sample_wait: Duration::from_millis(80),
            },
        );

        handle.join().unwrap();
        assert_eq!(status, StabilityStatus::Changing);
    }
}
