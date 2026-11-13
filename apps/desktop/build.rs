fn main() {
    slint_build::compile("ui/app.slint").expect("Slint UI compilation failed");
    println!("cargo:rerun-if-env-changed=UFFMPEG_EMBED_WINDOWS_ICON");
    println!("cargo:rerun-if-changed=../../packaging/windows.rc");
    println!("cargo:rerun-if-changed=../../packaging/icons/uffmpeg.ico");
    if std::env::var_os("UFFMPEG_EMBED_WINDOWS_ICON").is_some()
        && std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows")
    {
        assert_eq!(
            std::env::var("CARGO_CFG_TARGET_ENV").as_deref(),
            Ok("msvc"),
            "Release icon embedding requires MSVC"
        );
        let resources =
            std::path::PathBuf::from(std::env::var_os("OUT_DIR").unwrap()).join("uffmpeg.res");
        let packaging = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../packaging");
        let status = std::process::Command::new("rc.exe")
            .current_dir(packaging)
            .arg("/nologo")
            .arg("/fo")
            .arg(&resources)
            .arg("windows.rc")
            .status()
            .expect("Install the Windows SDK and run in a developer shell");
        assert!(status.success(), "Windows icon resource compilation failed");
        println!("cargo:rustc-link-arg={}", resources.display());
    }
}
