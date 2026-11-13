use crate::{MediaInfo, Stream};

#[derive(Debug, Clone, Copy, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MergeMode {
    Copy,
    Normalize,
}
impl MergeMode {
    pub fn extension(self) -> &'static str {
        match self {
            Self::Copy => "mkv",
            Self::Normalize => "mp4",
        }
    }
}
#[derive(Debug, Default)]
pub struct MergeCompatibility {
    pub issues: Vec<String>,
}
impl MergeCompatibility {
    pub fn can_copy(&self) -> bool {
        self.issues.is_empty()
    }
    pub fn message(&self) -> String {
        if self.can_copy() {
            "Fast-copy metadata checks passed. Clips will join in library order. Codec delay and source timestamps can leave small audio gaps or overlaps at joins; use normalization to rebuild the timeline.".into()
        } else {
            format!(
                "Fast copy is unavailable:\n{}\nChoose Normalize and re-encode to resolve stream differences.",
                self.issues.join("\n")
            )
        }
    }
}
/// Conservative concat-demuxer gate. Missing essential metadata cannot establish compatibility.
pub fn merge_compatibility(inputs: &[&MediaInfo]) -> MergeCompatibility {
    let mut report = MergeCompatibility::default();
    if inputs.len() < 2 {
        report.issues.push("Add at least two videos.".into());
        return report;
    }
    for (clip, info) in inputs.iter().enumerate() {
        let video_count = info
            .streams
            .iter()
            .filter(|s| s.codec_type.as_deref() == Some("video"))
            .count();
        if video_count != 1
            || !info.has_video()
            || info
                .streams
                .iter()
                .any(|s| !matches!(s.codec_type.as_deref(), Some("video" | "audio")))
        {
            report.issues.push(format!("Clip {}: fast copy needs one moving video stream and only audio streams alongside it.", clip + 1));
        }
        for stream in &info.streams {
            let missing = stream.codec_name.is_none()
                || !known_ratio(stream.time_base.as_deref())
                || match stream.codec_type.as_deref() {
                    Some("video") => {
                        stream.width.is_none()
                            || stream.height.is_none()
                            || stream.pix_fmt.is_none()
                            || !known_ratio(stream.avg_frame_rate.as_deref())
                            || stream.sample_aspect_ratio.is_none()
                    }
                    Some("audio") => {
                        stream.sample_rate.is_none()
                            || stream.channels.is_none()
                            || stream.channel_layout.is_none()
                            || stream.sample_fmt.is_none()
                    }
                    _ => true,
                };
            if missing {
                report.issues.push(format!(
                    "Clip {}, stream #{}: essential codec, timing or layout metadata is unknown.",
                    clip + 1,
                    stream.index
                ));
            }
            if stream.extradata_size.unwrap_or(0) > 0 && stream.extradata_hash.is_none() {
                report.issues.push(format!(
                    "Clip {}, stream #{}: codec configuration hash is missing.",
                    clip + 1,
                    stream.index
                ));
            }
            if stream
                .side_data_list
                .iter()
                .any(|d| d.rotation.is_some_and(|r| r != 0.0))
            {
                report.issues.push(format!(
                    "Clip {}: rotation metadata requires normalization.",
                    clip + 1
                ));
            }
        }
        if clip == 0 {
            continue;
        }
        let baseline = inputs[0];
        if info.streams.len() != baseline.streams.len() {
            report.issues.push(format!(
                "Clip {}: stream count differs from clip 1 ({} versus {}).",
                clip + 1,
                info.streams.len(),
                baseline.streams.len()
            ));
        }
        for (a, b) in baseline.streams.iter().zip(&info.streams) {
            let mut fields = Vec::new();
            macro_rules! compare {
                ($field:ident, $label:literal) => {
                    if a.$field != b.$field {
                        fields.push($label);
                    }
                };
            }
            compare!(index, "stream index");
            compare!(codec_type, "stream order/type");
            compare!(codec_name, "codec");
            compare!(profile, "profile");
            compare!(level, "level");
            compare!(time_base, "timebase");
            compare!(extradata_hash, "codec configuration");
            compare!(extradata_size, "codec configuration size");
            if a.codec_type.as_deref() == Some("video") {
                compare!(width, "width");
                compare!(height, "height");
                compare!(pix_fmt, "pixel format");
                compare!(sample_aspect_ratio, "pixel aspect ratio");
                compare!(avg_frame_rate, "frame rate");
                compare!(r_frame_rate, "nominal frame rate");
                compare!(field_order, "field order");
                compare!(color_range, "color range");
                compare!(color_space, "color space");
                compare!(color_transfer, "color transfer");
                compare!(color_primaries, "color primaries");
            }
            if a.codec_type.as_deref() == Some("audio") {
                compare!(sample_rate, "sample rate");
                compare!(sample_fmt, "sample format");
                compare!(channels, "channel count");
                compare!(channel_layout, "channel layout");
            }
            if !fields.is_empty() {
                report.issues.push(format!(
                    "Clip {}, stream #{} differs from clip 1: {}.",
                    clip + 1,
                    b.index,
                    fields.join(", ")
                ));
            }
        }
    }
    report
}
fn known_ratio(value: Option<&str>) -> bool {
    value.and_then(|s| s.split_once('/')).is_some_and(|(a, b)| {
        a.parse::<u64>().is_ok_and(|n| n > 0) && b.parse::<u64>().is_ok_and(|n| n > 0)
    })
}
impl MediaInfo {
    pub fn first_video(&self) -> Option<&Stream> {
        self.streams.iter().find(|s| {
            s.codec_type.as_deref() == Some("video")
                && s.disposition.as_ref().is_none_or(|d| d.attached_pic == 0)
        })
    }
    pub fn audio_count(&self) -> usize {
        self.streams
            .iter()
            .filter(|s| s.codec_type.as_deref() == Some("audio"))
            .count()
    }
}
/// A fixed normalization target shared by every clip in the merge.
#[derive(Debug, Clone, Copy)]
pub struct MergeTarget {
    pub width: u32,
    pub height: u32,
    pub audio_tracks: usize,
}
