use crate::AppWindow;
use frameforge_application::jobs::QueueHandle;
use slint::ComponentHandle;
use std::{path::Path, process::Command};

fn reveal(path: &Path) -> Result<(), String> {
    let path = path
        .canonicalize()
        .map_err(|e| format!("This file is no longer available: {e}"))?;
    #[cfg(target_os = "macos")]
    let mut command = {
        let mut c = Command::new("/usr/bin/open");
        c.arg("-R").arg(&path);
        c
    };
    #[cfg(windows)]
    let mut command = {
        let mut c = Command::new("explorer.exe");
        let mut selection = std::ffi::OsString::from("/select,");
        // canonicalize uses extended-length paths; Explorer expects ordinary drive/UNC syntax.
        use std::os::windows::ffi::{OsStrExt, OsStringExt};
        let wide = path.as_os_str().encode_wide().collect::<Vec<_>>();
        let display = if wide.starts_with(&[92, 92, 63, 92, 85, 78, 67, 92]) {
            let mut unc = vec![92, 92];
            unc.extend_from_slice(&wide[8..]);
            std::ffi::OsString::from_wide(&unc)
        } else if wide.starts_with(&[92, 92, 63, 92]) {
            std::ffi::OsString::from_wide(&wide[4..])
        } else {
            path.as_os_str().to_os_string()
        };
        selection.push(display);
        c.arg(selection);
        c
    };
    #[cfg(all(not(windows), not(target_os = "macos")))]
    let mut command = {
        let mut c = Command::new("xdg-open");
        c.arg(if path.is_dir() {
            path.as_path()
        } else {
            path.parent().ok_or("No parent folder")?
        });
        c
    };
    let status = command
        .status()
        .map_err(|e| format!("Could not open the file manager: {e}"))?;
    if !status.success() {
        return Err(format!("The file manager returned {status}"));
    }
    Ok(())
}
pub fn connect(ui: &AppWindow, queue: QueueHandle) {
    let weak = ui.as_weak();
    ui.on_reveal_licenses(move || {
        let Some(ui) = weak.upgrade() else { return; };
        let path = std::env::current_exe().ok().and_then(|exe| {
            let parent = exe.parent()?;
            [parent.join("../Resources/THIRD-PARTY.txt"), parent.join("THIRD-PARTY.txt")]
                .into_iter().find(|path| path.is_file())
        });
        let Some(path) = path else {
            ui.set_preferences_status("License records are included by the release packaging command. This development build has no collected records yet.".into());
            return;
        };
        let weak = ui.as_weak();
        std::thread::spawn(move || {
            if let Err(error) = reveal(&path) {
                let _ = weak.upgrade_in_event_loop(move |ui| ui.set_preferences_status(error.into()));
            }
        });
    });
    let weak = ui.as_weak();
    ui.on_reveal_output(move || {
        let Some(ui) = weak.upgrade() else {
            return;
        };
        let paths = ui
            .get_selected_job_id()
            .parse()
            .ok()
            .map(|id| queue.completed_paths(id))
            .unwrap_or_default();
        let Some(path) = paths.first().cloned() else {
            ui.set_import_status("This job has no saved output yet.".into());
            return;
        };
        let weak = ui.as_weak();
        std::thread::spawn(move || {
            if let Err(error) = reveal(&path) {
                let _ = weak.upgrade_in_event_loop(move |ui| ui.set_import_status(error.into()));
            }
        });
    });
}
