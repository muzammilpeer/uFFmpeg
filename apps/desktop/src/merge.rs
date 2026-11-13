use crate::{AppWindow, Library};
use frameforge_application::jobs::{JobRequest, QueueHandle};
use frameforge_application::{ImportedMedia, MergeMode, merge_overview};
use slint::ComponentHandle;

pub fn refresh(ui: &AppWindow, library: &[ImportedMedia]) {
    let overview = merge_overview(&library.iter().map(|item| &item.info).collect::<Vec<_>>());
    ui.set_can_merge(overview.can_merge);
    ui.set_can_copy_merge(overview.can_copy);
    ui.set_merge_summary(overview.summary.into());
}
pub fn connect(ui: &AppWindow, library: Library, queue: QueueHandle) {
    let weak = ui.as_weak();
    ui.on_export_merge(move || {
        let Some(ui) = weak.upgrade() else { return; };
        if ui.get_busy() || !ui.get_can_merge() { return; }
        let mode = if ui.get_normalize_merge() { MergeMode::Normalize } else { MergeMode::Copy };
        if matches!(mode, MergeMode::Copy) && !ui.get_can_copy_merge() {
            ui.set_import_status("These clips need normalization. Review the merge details and choose Normalize and re-encode.".into());
            return;
        }
        let paths = library.borrow().iter().map(|item| item.path.clone()).collect::<Vec<_>>();
        let Some(destination) = crate::preferences::dialog(true).set_title("Save merged video · existing files are protected")
            .add_filter("Merged video", &[mode.extension()]).set_file_name(format!("merged.{}", mode.extension())).save_file() else { return; };
        if let Some(folder) = destination.parent() { crate::preferences::remember(&ui, folder, true); }
        crate::jobs::submit(&ui, &queue, JobRequest::Merge { inputs: paths, destination, mode });
    });
}
