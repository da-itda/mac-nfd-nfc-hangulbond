fn main() {
    // Windows: tauri-build는 앱 실행 파일에만 공용 컨트롤 v6 manifest를 넣는다.
    // 그러면 cargo test 실행 파일이 STATUS_ENTRYPOINT_NOT_FOUND로 시작하지 못하므로,
    // 같은 manifest를 링커로 모든 실행 파일(앱·테스트)에 넣는다.
    let windows = std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows");
    let attributes = if windows {
        tauri_build::Attributes::new()
            .windows_attributes(tauri_build::WindowsAttributes::new_without_app_manifest())
    } else {
        tauri_build::Attributes::new()
    };
    if windows {
        let manifest = std::path::Path::new(&std::env::var("CARGO_MANIFEST_DIR").unwrap())
            .join("windows-app-manifest.xml");
        println!("cargo:rerun-if-changed={}", manifest.display());
        println!("cargo:rustc-link-arg=/MANIFEST:EMBED");
        println!("cargo:rustc-link-arg=/MANIFESTINPUT:{}", manifest.display());
    }
    tauri_build::try_build(attributes).expect("failed to run tauri-build");
}
