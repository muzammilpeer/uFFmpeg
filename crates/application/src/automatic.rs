use crate::{
    AudioFormat, AudioReport, SplitMode, SplitReport, StreamSelection, extract_audio,
    media_duration, split_media_selected,
};
use frameforge_domain::{auto_output_count, fixed_length_ranges, video_fixed_length_ranges};
use frameforge_engine::Toolchain;
use std::path::Path;

/// Recompute from the actual source at export time. Manual saved ranges are never changed.
pub fn auto_split_video(
    tools: &Toolchain,
    input: &Path,
    directory: &Path,
    segment_seconds: f64,
    mode: SplitMode,
    selection: Option<&StreamSelection>,
) -> SplitReport {
    let plan = tools
        .probe(input)
        .and_then(|info| video_fixed_length_ranges(&info, selection, segment_seconds));
    match plan {
        Ok(ranges) => split_media_selected(tools, input, directory, &ranges, mode, selection),
        Err(error) => SplitReport {
            completed: Vec::new(),
            error: Some(error),
        },
    }
}
pub fn auto_extract_audio(
    tools: &Toolchain,
    input: &Path,
    directory: &Path,
    audio_streams: &[u32],
    format: AudioFormat,
    segment_seconds: f64,
) -> AudioReport {
    let plan = tools
        .probe(input)
        .and_then(|info| media_duration(&info))
        .and_then(|duration| fixed_length_ranges(duration, segment_seconds))
        .and_then(|ranges| {
            auto_output_count(ranges.len(), audio_streams.len())?;
            Ok(ranges)
        });
    match plan {
        Ok(ranges) => extract_audio(
            tools,
            input,
            directory,
            audio_streams,
            format,
            Some(&ranges),
        ),
        Err(error) => AudioReport {
            completed: Vec::new(),
            error: Some(error),
        },
    }
}
