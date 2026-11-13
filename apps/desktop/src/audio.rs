use crate::{AppWindow, Library, StreamChoice};
use frameforge_application::jobs::{JobRequest, QueueHandle};
use frameforge_application::{AudioFormat, ImportedMedia};
use slint::{ComponentHandle, ModelRc, VecModel};

pub fn refresh(ui: &AppWindow, library: &[ImportedMedia], selected: i32) {
    let item = usize::try_from(selected).ok().and_then(|i| library.get(i));
    let mut video = Vec::new();
    let mut audio = Vec::new();
    if let Some(item) = item {
        for stream in &item.info.streams {
            let is_video = stream.codec_type.as_deref() == Some("video")
                && stream
                    .disposition
                    .as_ref()
                    .is_none_or(|d| d.attached_pic == 0);
            if !is_video && stream.codec_type.as_deref() != Some("audio") {
                continue;
            }
            let Ok(index) = i32::try_from(stream.index) else {
                continue;
            };
            let choice = StreamChoice {
                label: stream.label().into(),
                stream_index: index,
                chosen: if is_video {
                    item.selection.video == Some(stream.index)
                } else {
                    item.selection.audio.contains(&stream.index)
                },
            };
            if is_video {
                video.push(choice);
            } else {
                audio.push(choice);
            }
        }
    }
    let selected_audio = item.map_or(0, |i| i.selection.audio.len());
    let message = if item.is_none() {
        "Select a media file to choose its streams.".into()
    } else if audio.is_empty() {
        "This source has no audio streams. Choose another file for extraction.".into()
    } else if selected_audio == 0 {
        "No audio tracks selected. Video split will be silent; select audio for extraction.".into()
    } else {
        format!(
            "{selected_audio} audio track(s) selected. Extraction saves one file per track, for each range or full source."
        )
    };
    ui.set_has_selected_audio(selected_audio > 0);
    ui.set_audio_selection_status(message.into());
    ui.set_video_streams(ModelRc::new(VecModel::from(video)));
    ui.set_audio_streams(ModelRc::new(VecModel::from(audio)));
    crate::automatic::refresh(ui, library);
}
fn format(ui: &AppWindow) -> AudioFormat {
    match ui.get_audio_format() {
        1 => AudioFormat::Aac,
        2 => AudioFormat::Wav,
        _ => AudioFormat::Copy,
    }
}

pub fn connect(ui: &AppWindow, library: Library, queue: QueueHandle) {
    let (weak, items) = (ui.as_weak(), library.clone());
    ui.on_choose_video(move |index| {
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
            && let Ok(index) = u32::try_from(index)
            && item.info.streams.iter().any(|s| {
                s.index == index
                    && s.codec_type.as_deref() == Some("video")
                    && s.disposition.as_ref().is_none_or(|d| d.attached_pic == 0)
            })
        {
            item.selection.video = Some(index);
        }
        refresh(&ui, &items, ui.get_selected());
        crate::split::refresh(&ui, &items, ui.get_selected());
    });
    let (weak, items) = (ui.as_weak(), library.clone());
    ui.on_toggle_audio(move |index, checked| {
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
            && let Ok(index) = u32::try_from(index)
            && item
                .info
                .streams
                .iter()
                .any(|s| s.index == index && s.codec_type.as_deref() == Some("audio"))
        {
            item.selection.audio.retain(|i| *i != index);
            if checked {
                item.selection.audio.push(index);
                item.selection.audio.sort_unstable();
            }
        }
        refresh(&ui, &items, ui.get_selected());
    });
    let (weak, items) = (ui.as_weak(), library);
    ui.on_export_audio(move || {
        let Some(ui) = weak.upgrade() else {
            return;
        };
        if ui.get_busy() {
            return;
        }
        let full = ui.get_full_audio();
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
        let (path, selection, ranges) = {
            let items = items.borrow();
            let Some(item) = usize::try_from(ui.get_selected())
                .ok()
                .and_then(|i| items.get(i))
            else {
                return;
            };
            if item.info.audio_count() == 0 {
                ui.set_import_status("This source has no audio streams.".into());
                return;
            }
            if item.selection.audio.is_empty() {
                ui.set_import_status("Select at least one audio track before extraction.".into());
                return;
            }
            if !automatic && !full && item.ranges.is_empty() {
                ui.set_import_status("Add a range or choose Full source.".into());
                return;
            }
            (
                item.path.clone(),
                item.selection.clone(),
                item.ranges.clone(),
            )
        };
        let Some(directory) = crate::preferences::dialog(true)
            .set_title("Choose a folder for extracted audio")
            .pick_folder()
        else {
            return;
        };
        crate::preferences::remember(&ui, &directory, true);
        let format = format(&ui);
        crate::jobs::submit(
            &ui,
            &queue,
            JobRequest::Audio {
                input: path,
                directory,
                streams: selection.audio,
                format,
                ranges: if full { None } else { Some(ranges) },
                auto_seconds: seconds,
            },
        );
    });
}
