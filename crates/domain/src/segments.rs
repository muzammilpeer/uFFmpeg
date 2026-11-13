use crate::TimeRange;

pub const MAX_AUTO_OUTPUTS: usize = 10_000;
/// Shared parsing for desktop and CLI. A millisecond minimum avoids unrepresentable cuts.
pub fn parse_segment_seconds(value: &str, unit: &str) -> Result<f64, String> {
    let amount = value
        .trim()
        .parse::<f64>()
        .map_err(|_| "Enter a numeric segment length, such as 2 minutes or 30 seconds.")?;
    let multiplier = match unit.trim().to_ascii_lowercase().as_str() {
        "s" | "sec" | "second" | "seconds" => 1.0,
        "m" | "min" | "minute" | "minutes" => 60.0,
        _ => return Err("Choose seconds or minutes for the segment length.".into()),
    };
    let seconds = amount * multiplier;
    if !seconds.is_finite() || seconds < 0.001 {
        return Err("Segment length must be finite and at least 0.001 seconds.".into());
    }
    Ok(seconds)
}
/// Partition the whole source without cumulative float drift or an empty final range.
/// Microsecond ticks match probed duration precision; the final end retains the exact source end.
pub fn fixed_length_ranges(duration: f64, segment_seconds: f64) -> Result<Vec<TimeRange>, String> {
    TimeRange::new(0.0, duration, duration)?;
    if !segment_seconds.is_finite() || segment_seconds < 0.001 {
        return Err("Segment length must be finite and at least 0.001 seconds.".into());
    }
    if duration > (u64::MAX / 1_000_000) as f64 {
        return Err("Source duration is too large for automatic splitting.".into());
    }
    // Longer intervals intentionally produce just one whole-source clip.
    if segment_seconds >= duration {
        return Ok(vec![TimeRange::new(0.0, duration, duration)?]);
    }
    let total = (duration * 1_000_000.0).round().max(1.0) as u64;
    let step = (segment_seconds * 1_000_000.0).round() as u64;
    let count = total.div_ceil(step);
    if count > MAX_AUTO_OUTPUTS as u64 {
        return Err(format!(
            "This length would create {count} segments. Choose a longer length (maximum {MAX_AUTO_OUTPUTS} automatic outputs)."
        ));
    }
    (0..count)
        .map(|index| {
            let start = (index * step) as f64 / 1_000_000.0;
            let end = if index + 1 == count {
                duration
            } else {
                ((index + 1) * step) as f64 / 1_000_000.0
            };
            TimeRange::new(start, end, duration)
        })
        .collect()
}
pub fn auto_output_count(segments: usize, tracks: usize) -> Result<usize, String> {
    let count = segments
        .checked_mul(tracks)
        .ok_or("Too many automatic output files")?;
    if count > MAX_AUTO_OUTPUTS {
        return Err(format!(
            "This selection would create {count} files. Select fewer tracks or a longer segment length (maximum {MAX_AUTO_OUTPUTS} automatic outputs)."
        ));
    }
    Ok(count)
}

/// Video batches must contain a usable frame. Absorb a sub-frame final tail
/// into the preceding clip rather than publish an empty trailing file.
pub fn video_fixed_length_ranges(
    info: &crate::MediaInfo,
    selection: Option<&crate::StreamSelection>,
    segment_seconds: f64,
) -> Result<Vec<TimeRange>, String> {
    let selection = selection
        .cloned()
        .unwrap_or_else(|| crate::StreamSelection::for_source(info));
    selection.validate(info)?;
    let index = selection
        .video
        .ok_or("Select a moving video stream before automatic video splitting.")?;
    let video = info
        .streams
        .iter()
        .find(|s| s.index == index)
        .ok_or("Selected video stream is missing")?;
    let duration = info
        .format
        .as_ref()
        .and_then(|f| f.duration.as_deref())
        .and_then(|s| s.parse::<f64>().ok())
        .ok_or("Automatic splitting requires a known source duration.")?;
    let frame_seconds = [
        video.avg_frame_rate.as_deref(),
        video.r_frame_rate.as_deref(),
    ]
    .into_iter()
    .flatten()
    .find_map(|s| {
        let (a, b) = s.split_once('/')?;
        let (a, b) = (a.parse::<f64>().ok()?, b.parse::<f64>().ok()?);
        if a.is_finite() && b.is_finite() && a > 0.0 && b > 0.0 {
            Some(b / a)
        } else {
            None
        }
    });
    if let Some(frame) = frame_seconds
        && segment_seconds < duration
        && segment_seconds + 0.0000005 < frame
    {
        return Err(format!(
            "Segment length is shorter than a video frame. Choose at least {frame:.6} seconds for this stream."
        ));
    }
    let mut ranges = fixed_length_ranges(duration, segment_seconds)?;
    if let Some(frame) = frame_seconds
        && ranges.len() > 1
        && ranges
            .last()
            .is_some_and(|r| r.end - r.start + 0.0000005 < frame)
    {
        let tail = ranges.pop().ok_or("No final video segment")?;
        if let Some(last) = ranges.last_mut() {
            last.end = tail.end;
        }
    }
    Ok(ranges)
}
