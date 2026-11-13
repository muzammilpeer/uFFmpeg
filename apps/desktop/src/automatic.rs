use crate::{AppWindow, Library};
use frameforge_application::{
    ImportedMedia, auto_output_count, fixed_length_ranges, media_duration, parse_segment_seconds,
    video_fixed_length_ranges,
};
use slint::ComponentHandle;

pub fn interval(ui: &AppWindow) -> Result<f64, String> {
    parse_segment_seconds(
        ui.get_segment_length().as_str(),
        if ui.get_segment_unit() == 1 {
            "minutes"
        } else {
            "seconds"
        },
    )
}
pub fn refresh(ui: &AppWindow, library: &[ImportedMedia]) {
    let result = (|| {
        let item = usize::try_from(ui.get_selected())
            .ok()
            .and_then(|i| library.get(i))
            .ok_or("Select a source with a known duration to split automatically.")?;
        let seconds = interval(ui)?;
        let ranges = if ui.get_audio_view() {
            fixed_length_ranges(media_duration(&item.info)?, seconds)?
        } else {
            video_fixed_length_ranges(&item.info, Some(&item.selection), seconds)?
        };
        let count = if ui.get_audio_view() {
            auto_output_count(ranges.len(), item.selection.audio.len())?
        } else {
            ranges.len()
        };
        let last = ranges.last().ok_or("No source segments")?;
        let last_seconds = ((last.end - last.start) * 1_000_000.0).round() / 1_000_000.0;
        let summary = format!(
            "{} consecutive segments · final segment {last_seconds} seconds · {count} output file(s). The whole source is covered; saved ranges are kept separately.",
            ranges.len()
        );
        Ok::<_, String>((ranges.len() as i32, summary))
    })();
    match result {
        Ok((count, summary)) => {
            ui.set_auto_segment_count(count);
            ui.set_auto_summary(summary.into());
        }
        Err(error) => {
            ui.set_auto_segment_count(0);
            ui.set_auto_summary(error.into());
        }
    }
}
pub fn connect(ui: &AppWindow, library: Library) {
    let weak = ui.as_weak();
    ui.on_update_auto_plan(move || {
        if let Some(ui) = weak.upgrade() {
            refresh(&ui, &library.borrow());
        }
    });
}
