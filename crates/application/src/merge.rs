use crate::media_duration;
use frameforge_domain::{MediaInfo, MergeMode, MergeTarget, merge_compatibility};
use frameforge_engine::Toolchain;
use std::path::{Path, PathBuf};

pub struct MergeOverview {
    pub can_merge: bool,
    pub can_copy: bool,
    pub summary: String,
    pub target: Option<MergeTarget>,
}
pub fn merge_overview(infos: &[&MediaInfo]) -> MergeOverview {
    let mut errors = Vec::new();
    if infos.len() < 2 {
        errors.push("Add at least two videos to merge.".to_string());
    }
    for (index, info) in infos.iter().enumerate() {
        if !info.has_video() {
            errors.push(format!("Clip {} has no moving video. Remove audio-only items from the library before merging.", index + 1));
        }
        if media_duration(info).is_err() {
            errors.push(format!(
                "Clip {} needs a known positive duration.",
                index + 1
            ));
        }
    }
    let target = infos
        .first()
        .and_then(|info| info.first_video())
        .and_then(|video| {
            let mut width = f64::from(video.width?);
            let mut height = f64::from(video.height?);
            if let Some((a, b)) = video
                .sample_aspect_ratio
                .as_deref()
                .and_then(|s| s.split_once(':'))
                && let (Ok(a), Ok(b)) = (a.parse::<f64>(), b.parse::<f64>())
                && a > 0.0
                && b > 0.0
            {
                width *= a / b;
            }
            if video.side_data_list.iter().any(|data| {
                data.rotation
                    .is_some_and(|r| (r.abs() % 180.0 - 90.0).abs() < 0.01)
            }) {
                std::mem::swap(&mut width, &mut height);
            }
            if !width.is_finite()
                || !height.is_finite()
                || width <= 0.0
                || height <= 0.0
                || width > 16384.0
                || height > 16384.0
            {
                return None;
            }
            Some(MergeTarget {
                width: ((width.ceil() as u32) + 1) & !1,
                height: ((height.ceil() as u32) + 1) & !1,
                audio_tracks: infos
                    .iter()
                    .map(|info| info.audio_count())
                    .max()
                    .unwrap_or(0),
            })
        });
    if target.is_none() {
        errors.push("The first clip needs usable dimensions (at most 16384 pixels per side) for the normalization canvas.".into());
    }
    let can_merge = errors.is_empty();
    let compatibility = merge_compatibility(infos);
    let can_copy = can_merge && compatibility.can_copy();
    let mut summary = if can_merge {
        compatibility.message()
    } else {
        errors.join("\n")
    };
    if let Some(target) = target {
        summary.push_str(&format!("\nNormalize: {}×{}, 30 fps; {} audio track(s) at 48 kHz stereo, aligned by track order. Missing tracks become silence. Whole clips are used; split ranges are ignored. Normalization requires extra temporary disk space.", target.width, target.height, target.audio_tracks));
    }
    MergeOverview {
        can_merge,
        can_copy,
        summary,
        target,
    }
}

fn probe_inputs(
    tools: &Toolchain,
    inputs: &[PathBuf],
) -> Result<(Vec<PathBuf>, Vec<MediaInfo>), String> {
    if inputs.len() < 2 {
        return Err("Choose at least two video files.".into());
    }
    let mut paths = Vec::new();
    let mut infos = Vec::new();
    for (index, input) in inputs.iter().enumerate() {
        let path = input
            .canonicalize()
            .map_err(|e| format!("Clip {} cannot be opened: {e}", index + 1))?;
        if !path.is_file() {
            return Err(format!("Clip {} is not a regular file.", index + 1));
        }
        let info = tools
            .probe(&path)
            .map_err(|e| format!("Clip {} ({}): {e}", index + 1, path.display()))?;
        paths.push(path);
        infos.push(info);
    }
    Ok((paths, infos))
}
pub fn inspect_merge(tools: &Toolchain, inputs: &[PathBuf]) -> Result<MergeOverview, String> {
    let (_, infos) = probe_inputs(tools, inputs)?;
    Ok(merge_overview(&infos.iter().collect::<Vec<_>>()))
}

/// Single-output export with preflight, staging, validation and no-overwrite publication.
pub fn merge_media(
    tools: &Toolchain,
    inputs: &[PathBuf],
    destination: &Path,
    mode: MergeMode,
) -> Result<PathBuf, String> {
    let name = destination
        .file_name()
        .ok_or("Choose a file name for the merged video.")?;
    let extension = destination
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("");
    if !extension.eq_ignore_ascii_case(mode.extension()) {
        return Err(format!(
            "This merge mode needs a .{} output file.",
            mode.extension()
        ));
    }
    let parent = destination
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    let parent = parent
        .canonicalize()
        .map_err(|e| format!("Cannot open output folder: {e}"))?;
    if !parent.is_dir() {
        return Err("Choose an existing output folder.".into());
    }
    let destination = parent.join(name);
    match destination.symlink_metadata() {
        Ok(_) => {
            return Err(format!(
                "Output already exists: {}. Choose a new filename; existing files are never replaced.",
                destination.display()
            ));
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => (),
        Err(e) => return Err(format!("Cannot inspect the output path: {e}")),
    }
    let (paths, infos) = probe_inputs(tools, inputs)?;
    let overview = merge_overview(&infos.iter().collect::<Vec<_>>());
    if !overview.can_merge {
        return Err(overview.summary);
    }
    if matches!(mode, MergeMode::Copy) && !overview.can_copy {
        return Err(overview.summary);
    }
    let target = overview
        .target
        .ok_or("Normalization target is unavailable")?;
    let staging = tempfile::Builder::new()
        .prefix(".uffmpeg-merge-")
        .suffix(&format!(".{}", mode.extension()))
        .tempfile_in(&parent)
        .map_err(|e| format!("Cannot create output staging file: {e}"))?
        .into_temp_path();
    let expected_duration = infos
        .iter()
        .map(media_duration)
        .collect::<Result<Vec<_>, _>>()?
        .iter()
        .sum::<f64>();
    if !expected_duration.is_finite() {
        return Err("Combined duration is too large.".into());
    }
    let audio_tracks = match mode {
        MergeMode::Copy => {
            tools
                .control()
                .phase(0, 1, "Copying whole clips", Some(expected_duration));
            tools.concat_merge(&paths, &staging, false).map_err(|e| format!("Copy merge failed: {e}\nTry Normalize and re-encode if the source codecs cannot be stored in MKV."))?;
            infos[0].audio_count()
        }
        MergeMode::Normalize => {
            // Same filesystem as the output: avoid exhausting the system temp volume with video intermediates.
            let workspace = tempfile::Builder::new()
                .prefix(".uffmpeg-normalize-")
                .tempdir_in(&parent)
                .map_err(|e| e.to_string())?;
            let mut intermediates = Vec::new();
            for (index, (path, info)) in paths.iter().zip(&infos).enumerate() {
                tools.control().check()?;
                tools.control().phase(
                    index,
                    paths.len() + 1,
                    format!("Normalizing clip {} of {}", index + 1, paths.len()),
                    Some(media_duration(info)?),
                );
                let normalized = workspace.path().join(format!("clip-{index:06}.mkv"));
                tools
                    .normalize_merge_clip(path, info, &normalized, target, media_duration(info)?)
                    .map_err(|e| {
                        format!(
                            "Could not normalize clip {} ({}): {e}",
                            index + 1,
                            path.display()
                        )
                    })?;
                let normalized_info = tools.probe(&normalized)?;
                if !normalized_info.has_video()
                    || normalized_info.audio_count() != target.audio_tracks
                {
                    return Err(format!(
                        "Normalized clip {} has missing streams.",
                        index + 1
                    ));
                }
                intermediates.push(normalized);
            }
            tools.control().phase(
                paths.len(),
                paths.len() + 1,
                "Encoding merged output",
                Some(expected_duration),
            );
            tools
                .concat_merge(&intermediates, &staging, true)
                .map_err(|e| format!("Normalized merge failed: {e}"))?;
            target.audio_tracks
        }
    };
    let output = tools.probe(&staging)?;
    if !output.has_video() || output.audio_count() != audio_tracks {
        return Err("Merged output did not retain the expected video and audio streams.".into());
    }
    let duration = media_duration(&output)?;
    let tolerance = (inputs.len() as f64 * 0.25).max(expected_duration * 0.01);
    if (duration - expected_duration).abs() > tolerance {
        return Err(format!(
            "Merged duration {duration:.3}s differs too much from the expected {expected_duration:.3}s. No output was published."
        ));
    }
    tools.control().publish(&destination, || {
        staging.persist_noclobber(&destination).map_err(|e| {
            format!("Could not publish output without replacing an existing file: {e}")
        })
    })?;
    Ok(destination)
}
