// 운영체제마다 다른 작은 기능을 모은다: 홈 폴더, 콘솔 창 없이 외부 명령 실행, 링크 열기.
use std::path::PathBuf;
use std::process::Command;

pub fn home_dir() -> Result<PathBuf, String> {
    #[cfg(windows)]
    let variable = "USERPROFILE";
    #[cfg(not(windows))]
    let variable = "HOME";
    std::env::var_os(variable)
        .map(PathBuf::from)
        .ok_or_else(|| format!("{variable} is not set"))
}

// Windows에서 창 없는 앱이 콘솔 프로그램(curl 등)을 실행하면 검은 창이 잠깐 뜬다. 이를 막는다.
pub fn background_command(program: impl AsRef<std::ffi::OsStr>) -> Command {
    #[allow(unused_mut)]
    let mut command = Command::new(program);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        command.creation_flags(CREATE_NO_WINDOW);
    }
    command
}

pub fn open_url(url: &str) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    let mut command = {
        let mut command = background_command("open");
        command.arg(url);
        command
    };

    // cmd /C start는 콘솔 창이 깜빡이고 주소의 &를 명령 구분자로 읽는다. 기본 브라우저로 바로 연다.
    #[cfg(windows)]
    let mut command = {
        let mut command = background_command("rundll32.exe");
        command.args(["url.dll,FileProtocolHandler", url]);
        command
    };

    #[cfg(all(not(target_os = "macos"), not(windows)))]
    let mut command = {
        let mut command = background_command("xdg-open");
        command.arg(url);
        command
    };

    command
        .spawn()
        .map(|_| ())
        .map_err(|error| format!("failed to open {url}: {error}"))
}
