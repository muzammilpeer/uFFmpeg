use crate::{AppWindow, Library};
use frameforge_application::jobs::{JobRequest, QueueHandle};
use frameforge_application::{SplitMode, TimeRange, media_duration};
use slint::{ComponentHandle, ModelRc, VecModel};

pub fn refresh(ui: &AppWindow, library: &[frameforge_application::ImportedMedia], selected: i32) {
    let item = usize::try_from(selected).ok().and_then(|i| library.get(i));
    let duration = item.and_then(|item| media_duration(&item.info).ok());
    let can_split = item.is_some_and(|item| item.selection.video.is_some()) && duration.is_some();
    ui.set_can_range(item.is_some() && duration.is_some());
    ui.set_can_split(can_split);
    ui.set_range_start("0".into());
    ui.set_range_end(
        duration
            .map(|d| format!("{d:.6}"))
            .unwrap_or_default()
            .into(),
    );
    ui.set_ranges(ModelRc::new(VecModel::from(
        item.map(|item| {
            item.ranges
                .iter()
                .map(|r| format!("{:.6} → {:.6} seconds", r.start, r.end).into())
                .collect::<Vec<slint::SharedString>>()
        })
        .unwrap_or_default(),
    )));
}

pub fn connect(ui: &AppWindow, library: Library, queue: QueueHandle) {
    let (weak, items) = (ui.as_weak(), library.clone());
    ui.on_add_range(move || {
        let Some(ui) = weak.upgrade() else {
            return;
        };
        if ui.get_busy() || !ui.get_can_range() {
            return;
        }
        let result = (|| {
            let mut items = items.borrow_mut();
            let item = items
                .get_mut(
                    usize::try_from(ui.get_selected()).map_err(|_| "Select a media file first.")?,
                )
                .ok_or("Select a media file first.")?;
            let start = ui
                .get_range_start()
                .trim()
                .parse::<f64>()
                .map_err(|_| "Start must be a number of seconds, for example 1.25.")?;
            let end = ui
                .get_range_end()
                .trim()
                .parse::<f64>()
                .map_err(|_| "End must be a number of seconds, for example 5.5.")?;
            let range = TimeRange::new(start, end, media_duration(&item.info)?)?;
            item.ranges.push(range);
            refresh(&ui, &items, ui.get_selected());
            Ok::<_, String>(())
        })();
        ui.set_import_status(match result {
            Ok(()) => "Range added. Add more ranges, or export video/audio.".into(),
            Err(e) => e.into(),
        });
    });
    let (weak, items) = (ui.as_weak(), library.clone());
    ui.on_remove_range(move |index| {
        let Some(ui) = weak.upgrade() else {
            return;
        };
        if ui.get_busy() {
            return;
        }
        let mut items = items.borrow_mut();
        if let Some(item) = usize::try_from(ui.get_selected())
            .ok()
            .and_then(|i| items.get_mut(i))
            && let Ok(index) = usize::try_from(index)
            && index < item.ranges.len()
        {
            item.ranges.remove(index);
        }
        refresh(&ui, &items, ui.get_selected());
    });
    let (weak, items) = (ui.as_weak(), library);
    ui.on_export_segments(move || {
        let Some(ui) = weak.upgrade() else {
            return;
        };
        if ui.get_busy() || !ui.get_can_split() {
            return;
        }
        let automatic = ui.get_auto_split();
        let seconds = if automatic {
            match crate::automatic::interval(&ui) {
                Ok(seconds) => Some(seconds),
                Err(e) => {
                    ui.set_import_status(e.into());
                    return;
                }
            }
        } else {
            None
        };
        let (path, ranges, selection) = {
            let items = items.borrow();
            let Some(item) = usize::try_from(ui.get_selected())
                .ok()
                .and_then(|i| items.get(i))
            else {
                return;
            };
            if !automatic && item.ranges.is_empty() {
                ui.set_import_status("Add at least one range before exporting.".into());
                return;
            }
            (
                item.path.clone(),
                item.ranges.clone(),
                item.selection.clone(),
            )
        };
        let Some(directory) = crate::preferences::dialog(true)
            .set_title("Choose a folder for split clips")
            .pick_folder()
        else {
            return;
        };
        crate::preferences::remember(&ui, &directory, true);
        let mode = if ui.get_accurate() {
            SplitMode::Accurate
        } else {
            SplitMode::Copy
        };
        crate::jobs::submit(
            &ui,
            &queue,
            JobRequest::Split {
                input: path,
                directory,
                ranges,
                auto_seconds: seconds,
                mode,
                selection: Some(selection),
            },
        );
    });
}
