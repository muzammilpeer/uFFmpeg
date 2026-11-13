//! Small, versioned local preferences. Invalid files are preserved until an explicit save.
use crate::AppWindow;
use serde::{Deserialize, Serialize};
use slint::ComponentHandle;
use std::{
    io::Write,
    path::{Path, PathBuf},
};

#[derive(Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
struct Preferences {
    version: u32,
    accurate: bool,
    audio_format: i32,
    segment_length: String,
    segment_unit: i32,
    import_folder: Option<PathBuf>,
    output_folder: Option<PathBuf>,
}
impl Default for Preferences {
    fn default() -> Self {
        Self {
            version: 1,
            accurate: true,
            audio_format: 0,
            segment_length: "2".into(),
            segment_unit: 1,
            import_folder: None,
            output_folder: None,
        }
    }
}
fn path() -> PathBuf {
    crate::jobs::default_history().with_file_name("preferences.json")
}
fn load() -> Result<Preferences, String> {
    let path = path();
    if !path.exists() {
        return Ok(Preferences::default());
    }
    let file = std::fs::File::open(path).map_err(|e| e.to_string())?;
    if file.metadata().map_err(|e| e.to_string())?.len() > 65536 {
        return Err("Preferences file is too large".into());
    }
    let p: Preferences = serde_json::from_reader(file).map_err(|e| e.to_string())?;
    let length = p.segment_length.parse::<f64>().unwrap_or(0.0);
    if p.version != 1
        || !(0..=2).contains(&p.audio_format)
        || !(0..=1).contains(&p.segment_unit)
        || !length.is_finite()
        || length <= 0.0
    {
        return Err("Unsupported or invalid preferences".into());
    }
    Ok(p)
}
fn save(p: &Preferences) -> Result<(), String> {
    let path = path();
    let parent = path.parent().ok_or("Missing preferences directory")?;
    std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    let mut file = tempfile::NamedTempFile::new_in(parent).map_err(|e| e.to_string())?;
    serde_json::to_writer_pretty(&mut file, p).map_err(|e| e.to_string())?;
    file.flush()
        .and_then(|_| file.as_file().sync_all())
        .map_err(|e| e.to_string())?;
    file.persist(path).map_err(|e| e.to_string())?;
    Ok(())
}
pub fn dialog(output: bool) -> rfd::FileDialog {
    let prefs = load().unwrap_or_default();
    let folder = if output {
        prefs.output_folder
    } else {
        prefs.import_folder
    };
    let dialog = rfd::FileDialog::new();
    match folder.filter(|p| p.is_dir()) {
        Some(folder) => dialog.set_directory(folder),
        None => dialog,
    }
}
pub fn remember(ui: &AppWindow, folder: &Path, output: bool) {
    // Do not silently replace corrupt or future-version settings.
    let result = load().and_then(|mut p| {
        if output {
            p.output_folder = Some(folder.to_path_buf());
        } else {
            p.import_folder = Some(folder.to_path_buf());
        }
        save(&p)
    });
    if let Err(e) = result {
        ui.set_preferences_status(format!("Folder preference was not saved: {e}").into());
    }
}
pub fn connect(ui: &AppWindow) {
    match load() {
        Ok(p) => {
            ui.set_accurate(p.accurate);
            ui.set_audio_format(p.audio_format);
            ui.set_segment_length(p.segment_length.into());
            ui.set_segment_unit(p.segment_unit);
        }
        Err(e) => ui.set_preferences_status(
            format!("Using defaults. Existing settings were kept: {e}").into(),
        ),
    }
    let weak = ui.as_weak();
    ui.on_save_preferences(move || {
        let Some(ui) = weak.upgrade() else {
            return;
        };
        let length = ui.get_segment_length().parse::<f64>().unwrap_or(0.0);
        if !length.is_finite() || length <= 0.0 {
            ui.set_preferences_status("Enter a positive segment length before saving.".into());
            return;
        }
        // Refuse to overwrite malformed/future settings, even when saving other defaults.
        let result = load().and_then(|mut p| {
            p.accurate = ui.get_accurate();
            p.audio_format = ui.get_audio_format();
            p.segment_length = ui.get_segment_length().to_string();
            p.segment_unit = ui.get_segment_unit();
            save(&p)
        });
        ui.set_preferences_status(match result {
            Ok(()) => {
                "Defaults saved for your next session. Folder choices are remembered automatically."
                    .into()
            }
            Err(e) => format!("Could not save preferences: {e}").into(),
        });
    });
}
