// Chrome 웹 스토어의 한글본드 확장을 Chrome 외부 확장으로 등록한다.
// 등록해 두면 Chrome이 다음 실행 때 사용자에게 사용 설정을 묻는다. 몰래 켜지는 않는다.
// - macOS: Chrome 데이터 폴더의 External Extensions/<id>.json 파일
// - Windows: 레지스트리 HKCU\Software\Google\Chrome\Extensions\<id> 의 update_url 값
// macOS 27부터는 다른 앱의 데이터 폴더 접근이 막혀 등록할 수 없을 수 있다. 그때는 웹 스토어에서 설치하게 한다.
use std::fs;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};

use serde::Serialize;

use crate::os::{background_command, open_url};

pub const EXTENSION_ID: &str = "hmgopkkefgfppfedloeponobiamikomf";
pub const STORE_URL: &str =
    "https://chromewebstore.google.com/detail/hmgopkkefgfppfedloeponobiamikomf";
const UPDATE_URL: &str = "https://clients2.google.com/service/update2/crx";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum ExtensionState {
    NoChrome,
    Installed,
    Registered,
    NotPublished,
    NotRegistered,
    StoreInstall,
}

pub fn status(home: &Path, check_store: bool) -> ExtensionState {
    if platform::chrome_executable(home).is_none() {
        return ExtensionState::NoChrome;
    }
    match installed_in_profile(home) {
        Ok(true) => return ExtensionState::Installed,
        Ok(false) => {}
        Err(_) => {
            if check_store && !published() {
                return ExtensionState::NotPublished;
            }
            return ExtensionState::StoreInstall;
        }
    }
    if platform::is_registered(home) {
        return ExtensionState::Registered;
    }
    if check_store && !published() {
        return ExtensionState::NotPublished;
    }
    ExtensionState::NotRegistered
}

// 이미 설치·등록되어 있거나 아직 공개 전이면 아무것도 하지 않는다.
// 사용자가 Chrome에서 확장을 지운 경우에는 등록이 남아 있으므로 다시 묻지 않는다.
pub fn register(home: &Path) -> Result<ExtensionState, String> {
    register_with(home, true)
}

fn register_with(home: &Path, check_store: bool) -> Result<ExtensionState, String> {
    let state = status(home, check_store);
    if state != ExtensionState::NotRegistered {
        return Ok(state);
    }
    match platform::write_registration(home) {
        Ok(()) => Ok(ExtensionState::Registered),
        Err(error) if error.kind() == ErrorKind::PermissionDenied => Ok(ExtensionState::StoreInstall),
        Err(error) => Err(format!("failed to register the Chrome extension: {error}")),
    }
}

// 기본 브라우저가 다른 브라우저여도 Chrome에서 열어야 확장을 추가할 수 있다.
pub fn open_store() -> Result<(), String> {
    let home = crate::os::home_dir()?;
    if platform::open_in_chrome(&home, STORE_URL) {
        return Ok(());
    }
    open_url(STORE_URL)
}

// Chrome 데이터 폴더를 읽을 수 없으면 오류를 돌려준다. 아직 Chrome을 한 번도 열지 않았으면 미설치로 본다.
fn installed_in_profile(home: &Path) -> std::io::Result<bool> {
    let entries = match fs::read_dir(platform::chrome_data_dir(home)) {
        Ok(entries) => entries,
        Err(error) if error.kind() == ErrorKind::NotFound => return Ok(false),
        Err(error) => return Err(error),
    };
    Ok(entries
        .flatten()
        .any(|entry| entry.path().join("Extensions").join(EXTENSION_ID).is_dir()))
}

// 웹 스토어에 공개된 확장만 업데이트 서버가 302로 내려받기 주소를 준다.
fn published() -> bool {
    let url = format!(
        "{UPDATE_URL}?response=redirect&prodversion=130.0&acceptformat=crx3&x=id%3D{EXTENSION_ID}%26uc"
    );
    #[cfg(windows)]
    let (curl, null) = ("curl.exe", "NUL");
    #[cfg(not(windows))]
    let (curl, null) = ("/usr/bin/curl", "/dev/null");
    background_command(curl)
        .args(["-s", "-o", null, "--max-time", "10", "-w", "%{http_code}", &url])
        .output()
        .map(|output| output.stdout == b"302")
        .unwrap_or(false)
}

#[cfg(not(windows))]
mod platform {
    use super::*;

    pub fn chrome_executable(home: &Path) -> Option<PathBuf> {
        [
            PathBuf::from("/Applications/Google Chrome.app"),
            home.join("Applications/Google Chrome.app"),
        ]
        .into_iter()
        .find(|path| path.exists())
    }

    pub fn chrome_data_dir(home: &Path) -> PathBuf {
        home.join("Library/Application Support/Google/Chrome")
    }

    pub fn registration_file(home: &Path) -> PathBuf {
        chrome_data_dir(home)
            .join("External Extensions")
            .join(format!("{EXTENSION_ID}.json"))
    }

    pub fn is_registered(home: &Path) -> bool {
        registration_file(home).exists()
    }

    pub fn write_registration(home: &Path) -> std::io::Result<()> {
        let file = registration_file(home);
        if let Some(parent) = file.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(
            &file,
            format!("{{\n  \"external_update_url\": \"{UPDATE_URL}\"\n}}\n"),
        )
    }

    pub fn open_in_chrome(_home: &Path, url: &str) -> bool {
        background_command("/usr/bin/open")
            .args(["-a", "Google Chrome", url])
            .status()
            .map(|status| status.success())
            .unwrap_or(false)
    }
}

#[cfg(windows)]
mod platform {
    use super::*;
    use winreg::enums::HKEY_CURRENT_USER;
    use winreg::RegKey;

    // 테스트는 실제 Chrome 등록을 건드리지 않도록 별도 키를 쓴다.
    #[cfg(not(test))]
    fn registry_key() -> String {
        format!(r"Software\Google\Chrome\Extensions\{EXTENSION_ID}")
    }

    #[cfg(test)]
    fn registry_key() -> String {
        format!(
            r"Software\HangulBondTest\{}\Extensions\{EXTENSION_ID}",
            std::process::id()
        )
    }

    pub fn chrome_executable(home: &Path) -> Option<PathBuf> {
        let relative = r"Google\Chrome\Application\chrome.exe";
        let mut candidates = vec![home.join(r"AppData\Local").join(relative)];
        // 테스트는 임시 홈 폴더만 보도록 시스템 설치 경로를 확인하지 않는다.
        if !cfg!(test) {
            for variable in ["ProgramFiles", "ProgramFiles(x86)", "LOCALAPPDATA"] {
                if let Some(base) = std::env::var_os(variable) {
                    candidates.push(PathBuf::from(base).join(relative));
                }
            }
        }
        candidates.into_iter().find(|path| path.is_file())
    }

    pub fn chrome_data_dir(home: &Path) -> PathBuf {
        home.join(r"AppData\Local\Google\Chrome\User Data")
    }

    pub fn is_registered(_home: &Path) -> bool {
        RegKey::predef(HKEY_CURRENT_USER)
            .open_subkey(registry_key())
            .is_ok()
    }

    pub fn write_registration(_home: &Path) -> std::io::Result<()> {
        let (key, _) = RegKey::predef(HKEY_CURRENT_USER).create_subkey(registry_key())?;
        key.set_value("update_url", &UPDATE_URL)
    }

    pub fn open_in_chrome(home: &Path, url: &str) -> bool {
        chrome_executable(home)
            .and_then(|chrome| background_command(chrome).arg(url).spawn().ok())
            .is_some()
    }

    #[cfg(test)]
    pub fn remove_test_registration() {
        let _ = RegKey::predef(HKEY_CURRENT_USER).delete_subkey_all(r"Software\HangulBondTest");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_home(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("hangulbond-ext-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        #[cfg(not(windows))]
        fs::create_dir_all(dir.join("Applications/Google Chrome.app")).unwrap();
        #[cfg(windows)]
        {
            let chrome = dir.join(r"AppData\Local\Google\Chrome\Application\chrome.exe");
            fs::create_dir_all(chrome.parent().unwrap()).unwrap();
            fs::write(chrome, "").unwrap();
        }
        dir
    }

    #[test]
    fn detects_extension_installed_in_any_profile() {
        let home = temp_home("installed");
        fs::create_dir_all(
            platform::chrome_data_dir(&home)
                .join("Profile 2/Extensions")
                .join(EXTENSION_ID),
        )
        .unwrap();
        assert_eq!(status(&home, false), ExtensionState::Installed);
        assert_eq!(register(&home).unwrap(), ExtensionState::Installed);
        assert!(!platform::is_registered(&home));
        fs::remove_dir_all(home).unwrap();
    }

    #[test]
    fn reports_missing_chrome() {
        let home = std::env::temp_dir().join(format!("hangulbond-ext-nochrome-{}", std::process::id()));
        fs::create_dir_all(&home).unwrap();
        if platform::chrome_executable(&home).is_none() {
            assert_eq!(status(&home, false), ExtensionState::NoChrome);
        }
        fs::remove_dir_all(home).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn falls_back_to_store_when_chrome_data_is_not_readable() {
        use std::os::unix::fs::PermissionsExt;
        let home = temp_home("denied");
        let data = platform::chrome_data_dir(&home);
        fs::create_dir_all(&data).unwrap();
        fs::set_permissions(&data, fs::Permissions::from_mode(0o000)).unwrap();
        let unreadable = status(&home, false);
        // 읽을 수는 있지만 쓸 수 없는 경우
        fs::set_permissions(&data, fs::Permissions::from_mode(0o555)).unwrap();
        let unwritable = register_with(&home, false);
        fs::set_permissions(&data, fs::Permissions::from_mode(0o755)).unwrap();
        assert_eq!(unreadable, ExtensionState::StoreInstall);
        assert_eq!(unwritable, Ok(ExtensionState::StoreInstall));
        assert!(!platform::is_registered(&home));
        fs::remove_dir_all(home).unwrap();
    }

    #[cfg(not(windows))]
    #[test]
    fn keeps_existing_registration() {
        let home = temp_home("registered");
        let file = platform::registration_file(&home);
        fs::create_dir_all(file.parent().unwrap()).unwrap();
        fs::write(&file, "{}").unwrap();
        assert_eq!(register(&home).unwrap(), ExtensionState::Registered);
        assert_eq!(fs::read_to_string(&file).unwrap(), "{}");
        fs::remove_dir_all(home).unwrap();
    }

    #[cfg(windows)]
    #[test]
    fn registers_in_the_registry_once() {
        let home = temp_home("registry");
        platform::remove_test_registration();
        assert_eq!(register_with(&home, false).unwrap(), ExtensionState::Registered);
        assert!(platform::is_registered(&home));
        assert_eq!(register_with(&home, false).unwrap(), ExtensionState::Registered);
        platform::remove_test_registration();
        assert!(!platform::is_registered(&home));
        fs::remove_dir_all(home).unwrap();
    }
}
