//! UI-independent application service; blocking calls belong on a worker.
mod audio;
mod automatic;
pub mod jobs;
pub use automatic::{auto_extract_audio, auto_split_video};
pub use frameforge_domain::{
    auto_output_count, fixed_length_ranges, parse_segment_seconds, video_fixed_length_ranges,
};
mod merge;
pub use audio::{AudioReport, extract_audio};
pub use frameforge_domain::MergeMode;
pub use frameforge_domain::{AudioFormat, StreamSelection};
use frameforge_engine::Toolchain;
pub use merge::{MergeOverview, inspect_merge, merge_media, merge_overview};
pub fn diagnostics() -> Result<String, String> {
    let chain = Toolchain::discover()?;
    let (ffmpeg, ffprobe) = chain.versions()?;
    Ok(format!("Ready · {}\n{}\n{}", chain.source, ffmpeg, ffprobe))
}

use frameforge_domain::MediaInfo;
use std::path::{Path, PathBuf};
/// Owns the preview directory for the lifetime of this imported item.
pub struct ImportedMedia {
    pub path: PathBuf,
    pub info: MediaInfo,
    pub summary: String,
    pub thumbnail: Option<PathBuf>,
    pub warning: Option<String>,
    pub ranges: Vec<frameforge_domain::TimeRange>,
    pub selection: StreamSelection,
    _cache: tempfile::TempDir,
}
pub fn import_media(tools: &Toolchain, path: &Path) -> Result<ImportedMedia, String> {
    let path = path
        .canonicalize()
        .map_err(|e| format!("Cannot open file: {e}"))?;
    if !path.is_file() {
        return Err("Choose a regular video or audio file, not a folder.".into());
    }
    let file = std::fs::File::open(&path).map_err(|e| format!("Cannot read file: {e}"))?;
    if file.metadata().map_err(|e| e.to_string())?.len() == 0 {
        return Err("This file is empty. Choose a video or audio file.".into());
    }
    let info = tools
        .probe(&path)
        .map_err(|e| format!("Cannot inspect this media. It may be unsupported or damaged. {e}"))?;
    if !info
        .streams
        .iter()
        .any(|s| matches!(s.codec_type.as_deref(), Some("audio" | "video")))
    {
        return Err("No video or audio streams found. Choose a playable media file.".into());
    }
    let cache = tempfile::tempdir().map_err(|e| format!("Cannot create preview cache: {e}"))?;
    let mut thumbnail = None;
    let mut warning = None;
    if info
        .streams
        .iter()
        .any(|s| s.codec_type.as_deref() == Some("video"))
    {
        let output = cache.path().join("preview.png");
        match tools.thumbnail(&path, &output) {
            Ok(()) => thumbnail = Some(output),
            Err(e) => warning = Some(format!("Imported without a preview: {e}")),
        }
    }
    let mut summary = String::new();
    if let Some(format) = &info.format {
        summary.push_str(&format!(
            "Container: {}\nDuration: {} seconds\n",
            format.format_name.as_deref().unwrap_or("Unknown"),
            format.duration.as_deref().unwrap_or("Unknown")
        ));
    }
    for stream in &info.streams {
        summary.push_str(&format!(
            "\nStream #{} · {} · {}",
            stream.index,
            stream.codec_type.as_deref().unwrap_or("Unknown"),
            stream.codec_name.as_deref().unwrap_or("Unknown")
        ));
        if let (Some(w), Some(h)) = (stream.width, stream.height) {
            summary.push_str(&format!(" · {w}×{h}"));
        }
        if let Some(rate) = &stream.avg_frame_rate
            && rate != "0/0"
        {
            summary.push_str(&format!(" · {rate} fps"));
        }
        if let Some(rate) = &stream.sample_rate {
            summary.push_str(&format!(" · {rate} Hz"));
        }
        if let Some(channels) = stream.channels {
            summary.push_str(&format!(" · {channels} channels"));
        }
    }
    let selection = StreamSelection::for_source(&info);
    Ok(ImportedMedia {
        path,
        info,
        summary,
        thumbnail,
        warning,
        ranges: Vec::new(),
        selection,
        _cache: cache,
    })
}

pub use frameforge_domain::{SplitMode, TimeRange};
pub fn media_duration(info: &MediaInfo) -> Result<f64, String> {
    info.format
        .as_ref()
        .and_then(|f| f.duration.as_deref())
        .and_then(|s| s.parse::<f64>().ok())
        .filter(|d| d.is_finite() && *d > 0.0)
        .ok_or_else(|| "Media duration is unknown; this file cannot be exported yet.".into())
}
#[derive(Debug, Default)]
pub struct SplitReport {
    pub completed: Vec<PathBuf>,
    pub error: Option<String>,
}
impl SplitReport {
    pub fn message(&self) -> String {
        let mut message = format!("Saved {} segment(s).", self.completed.len());
        for path in &self.completed {
            message.push_str(&format!("\n{}", path.display()));
        }
        if let Some(error) = &self.error {
            message.push_str(&format!("\nStopped: {error}"));
        }
        message
    }
}
/// Re-probe at export time, preflight every destination, and publish each validated clip without overwrite.
pub fn split_media(
    tools: &Toolchain,
    input: &Path,
    directory: &Path,
    ranges: &[TimeRange],
    mode: SplitMode,
) -> SplitReport {
    split_media_selected(tools, input, directory, ranges, mode, None)
}

pub fn split_media_selected(
    tools: &Toolchain,
    input: &Path,
    directory: &Path,
    ranges: &[TimeRange],
    mode: SplitMode,
    selection: Option<&StreamSelection>,
) -> SplitReport {
    let mut report = SplitReport::default();
    let result = (|| -> Result<(), String> {
        if ranges.is_empty() {
            return Err("Add at least one range before exporting.".into());
        }
        let input = input
            .canonicalize()
            .map_err(|e| format!("Cannot read source: {e}"))?;
        if !input.is_file() {
            return Err("The source is not a regular file.".into());
        }
        let info = tools.probe(&input)?;
        if !info.has_video() {
            return Err("Select a file containing moving video, not audio with cover art.".into());
        }
        let selection = selection
            .cloned()
            .unwrap_or_else(|| StreamSelection::for_source(&info));
        selection.validate(&info)?;
        if selection.video.is_none() {
            return Err("Select a video stream before splitting.".into());
        }
        let duration = media_duration(&info)?;
        let directory = directory
            .canonicalize()
            .map_err(|e| format!("Cannot open output folder: {e}"))?;
        if !directory.is_dir() {
            return Err("Choose an existing output folder.".into());
        }
        let stem = input.file_stem().ok_or("Source has no file name")?;
        let mut outputs = Vec::new();
        for (index, range) in ranges.iter().enumerate() {
            TimeRange::new(range.start, range.end, duration)?;
            let mut name = stem.to_os_string();
            name.push(format!("-segment-{:03}.{}", index + 1, mode.extension()));
            let output = directory.join(name);
            // symlink_metadata also detects dangling symlinks and directories.
            match output.symlink_metadata() {
                Ok(_) => {
                    return Err(format!(
                        "Output already exists: {}. Choose another folder or move that file.",
                        output.display()
                    ));
                }
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => (),
                Err(e) => return Err(format!("Cannot inspect output path: {e}")),
            }
            outputs.push(output);
        }
        for (part, (range, output)) in ranges.iter().zip(outputs).enumerate() {
            tools.control().check()?;
            tools.control().phase(
                part,
                ranges.len(),
                format!("Video segment {} of {}", part + 1, ranges.len()),
                Some(range.end - range.start),
            );
            let staging = tempfile::Builder::new()
                .prefix(".uffmpeg-split-")
                .suffix(&format!(".{}", mode.extension()))
                .tempfile_in(&directory)
                .map_err(|e| format!("Cannot write output folder: {e}"))?
                .into_temp_path();
            tools
                .split_segment(&input, &staging, *range, mode, &selection)
                .map_err(|e| {
                    let hint = match mode {
                        SplitMode::Copy => {
                            " Try accurate mode if the source codecs cannot be copied into MKV."
                        }
                        SplitMode::Accurate => {
                            " Confirm this FFmpeg build includes the MPEG-4 video and AAC encoders."
                        }
                    };
                    format!("Could not export {}: {e}{hint}", output.display())
                })?;
            let result = tools.probe(&staging)?;
            if !result
                .streams
                .iter()
                .any(|s| s.codec_type.as_deref() == Some("video"))
                || media_duration(&result)? <= 0.0
            {
                return Err("Export did not produce a nonempty video clip. Try a longer range or accurate mode.".into());
            }
            let expected_audio = selection.audio.len();
            if result
                .streams
                .iter()
                .filter(|s| s.codec_type.as_deref() == Some("audio"))
                .count()
                != expected_audio
            {
                return Err("Export did not preserve all audio streams. Try a longer range or another mode.".into());
            }
            tools.control().publish(&output, || {
                staging.persist_noclobber(&output).map_err(|e| {
                    format!(
                        "Could not publish {} without overwrite: {e}",
                        output.display()
                    )
                })
            })?;
            report.completed.push(output);
        }
        Ok(())
    })();
    report.error = result.err();
    report
}
