use crate::media_duration;
use frameforge_domain::{AudioFormat, StreamSelection, TimeRange};
use frameforge_engine::Toolchain;
use std::path::{Path, PathBuf};
#[derive(Debug, Default)]
pub struct AudioReport {
    pub completed: Vec<PathBuf>,
    pub error: Option<String>,
}
impl AudioReport {
    pub fn message(&self) -> String {
        let mut text = format!("Saved {} audio file(s).", self.completed.len());
        for path in &self.completed {
            text.push_str(&format!("\n{}", path.display()));
        }
        if let Some(error) = &self.error {
            text.push_str(&format!("\nStopped: {error}"));
        }
        text
    }
}
/// None ranges means full source. Some(empty) is invalid, never an implicit full-file export.
pub fn extract_audio(
    tools: &Toolchain,
    input: &Path,
    directory: &Path,
    audio_streams: &[u32],
    format: AudioFormat,
    ranges: Option<&[TimeRange]>,
) -> AudioReport {
    let mut report = AudioReport::default();
    let result = (|| -> Result<(), String> {
        let input = input
            .canonicalize()
            .map_err(|e| format!("Cannot read source: {e}"))?;
        if !input.is_file() {
            return Err("Choose a regular media file.".into());
        }
        let info = tools.probe(&input)?;
        if info.audio_count() == 0 {
            return Err("This source has no audio streams. Choose a file containing audio.".into());
        }
        if audio_streams.is_empty() {
            return Err("Select at least one audio track before extraction.".into());
        }
        StreamSelection {
            video: None,
            audio: audio_streams.to_vec(),
        }
        .validate(&info)?;
        for index in audio_streams {
            let stream = info
                .streams
                .iter()
                .find(|s| s.index == *index)
                .ok_or("Selected stream disappeared")?;
            format.validate_codec(stream)?;
        }
        let scopes = match ranges {
            None => vec![None],
            Some(ranges) => {
                if ranges.is_empty() {
                    return Err("Add at least one range, or choose Full source.".into());
                }
                let duration = media_duration(&info)?;
                ranges
                    .iter()
                    .map(|r| TimeRange::new(r.start, r.end, duration).map(Some))
                    .collect::<Result<Vec<_>, _>>()?
            }
        };
        let directory = directory
            .canonicalize()
            .map_err(|e| format!("Cannot open output folder: {e}"))?;
        if !directory.is_dir() {
            return Err("Choose an existing output folder.".into());
        }
        let stem = input.file_stem().ok_or("Source has no filename")?;
        let mut jobs = Vec::new();
        // Preflight every name before starting any output, including later tracks and ranges.
        for (range_index, range) in scopes.iter().enumerate() {
            for stream_index in audio_streams {
                let mut name = stem.to_os_string();
                let scope = if range.is_some() {
                    format!("range-{:03}", range_index + 1)
                } else {
                    "full".into()
                };
                name.push(format!(
                    "-audio-s{stream_index:03}-{scope}.{}",
                    format.extension()
                ));
                let destination = directory.join(name);
                match destination.symlink_metadata() {
                    Ok(_) => {
                        return Err(format!(
                            "Output already exists: {}. Choose another folder; existing files are never replaced.",
                            destination.display()
                        ));
                    }
                    Err(e) if e.kind() == std::io::ErrorKind::NotFound => (),
                    Err(e) => return Err(format!("Cannot inspect output path: {e}")),
                }
                jobs.push((*stream_index, *range, destination));
            }
        }
        let total_jobs = jobs.len();
        for (part, (index, range, destination)) in jobs.into_iter().enumerate() {
            tools.control().check()?;
            tools.control().phase(
                part,
                total_jobs,
                format!(
                    "Audio file {} of {} · source stream #{index}",
                    part + 1,
                    total_jobs
                ),
                range
                    .map(|r| r.end - r.start)
                    .or_else(|| media_duration(&info).ok()),
            );
            let staging = tempfile::Builder::new()
                .prefix(".uffmpeg-audio-")
                .suffix(&format!(".{}", format.extension()))
                .tempfile_in(&directory)
                .map_err(|e| format!("Cannot write output folder: {e}"))?
                .into_temp_path();
            tools.extract_audio_stream(&input, &staging, index, format, range).map_err(|e| format!("Could not extract stream #{index}: {e}. Choose another format if the selected codec is unsupported."))?;
            let output = tools.probe(&staging)?;
            if output.streams.len() != 1
                || output.audio_count() != 1
                || media_duration(&output).is_err()
            {
                return Err(format!(
                    "Stream #{index} produced no usable audio in the chosen interval. Try a longer range or check the track's timing."
                ));
            }
            if let Some(range) = range
                && media_duration(&output)? > range.end - range.start + 0.25
            {
                return Err("Exported audio extends beyond the selected range. Choose AAC or WAV conversion for tighter cuts.".into());
            }
            let source = info
                .streams
                .iter()
                .find(|s| s.index == index)
                .ok_or("Selected stream disappeared")?;
            let audio = &output.streams[0];
            let expected_codec = match format {
                AudioFormat::Copy => source.codec_name.as_deref(),
                AudioFormat::Aac => Some("aac"),
                AudioFormat::Wav => Some("pcm_s16le"),
            };
            if audio.codec_name.as_deref() != expected_codec {
                return Err("The exported audio codec does not match the chosen format.".into());
            }
            if source.channels.is_some() && audio.channels != source.channels {
                return Err(
                    "Export unexpectedly changed the selected track's channel count.".into(),
                );
            }
            if matches!(format, AudioFormat::Copy | AudioFormat::Wav)
                && source.sample_rate.is_some()
                && audio.sample_rate != source.sample_rate
            {
                return Err("Export unexpectedly changed the selected track's sample rate.".into());
            }
            tools.control().publish(&destination, || {
                staging.persist_noclobber(&destination).map_err(|e| {
                    format!(
                        "Could not publish {} without overwrite: {e}",
                        destination.display()
                    )
                })
            })?;
            report.completed.push(destination);
        }
        Ok(())
    })();
    report.error = result.err();
    report
}
