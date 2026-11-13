//! Media metadata shared independently of UI and process execution.
mod merge;
mod segments;
mod streams;
pub use merge::{MergeCompatibility, MergeMode, MergeTarget, merge_compatibility};
pub use segments::{
    MAX_AUTO_OUTPUTS, auto_output_count, fixed_length_ranges, parse_segment_seconds,
    video_fixed_length_ranges,
};
use serde::Deserialize;
pub use streams::{AudioFormat, StreamSelection};
#[derive(Debug, Deserialize)]
pub struct MediaInfo {
    #[serde(default)]
    pub streams: Vec<Stream>,
    pub format: Option<Format>,
}
#[derive(Debug, Deserialize)]
pub struct Stream {
    pub index: u32,
    pub codec_type: Option<String>,
    pub codec_name: Option<String>,
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub sample_rate: Option<String>,
    pub channels: Option<u32>,
    pub avg_frame_rate: Option<String>,
    pub disposition: Option<StreamDisposition>,
    pub tags: Option<StreamTags>,
    pub time_base: Option<String>,
    pub profile: Option<String>,
    pub level: Option<i32>,
    pub pix_fmt: Option<String>,
    pub sample_fmt: Option<String>,
    pub sample_aspect_ratio: Option<String>,
    pub r_frame_rate: Option<String>,
    pub channel_layout: Option<String>,
    pub field_order: Option<String>,
    pub color_range: Option<String>,
    pub color_space: Option<String>,
    pub color_transfer: Option<String>,
    pub color_primaries: Option<String>,
    pub extradata_size: Option<u64>,
    pub extradata_hash: Option<String>,
    #[serde(default)]
    pub side_data_list: Vec<StreamSideData>,
}
#[derive(Debug, Deserialize)]
pub struct Format {
    pub duration: Option<String>,
    pub format_name: Option<String>,
}

/// A validated half-open range on the source timeline, in seconds.
#[derive(Debug, Clone, Copy, serde::Serialize, serde::Deserialize)]
pub struct TimeRange {
    pub start: f64,
    pub end: f64,
}
impl TimeRange {
    pub fn new(start: f64, end: f64, duration: f64) -> Result<Self, String> {
        if !duration.is_finite() || duration <= 0.0 {
            return Err("A known, positive media duration is required.".into());
        }
        if !start.is_finite() || !end.is_finite() || start < 0.0 || end <= start || end > duration {
            return Err(format!("Use 0 ≤ start < end ≤ {duration:.6} seconds."));
        }
        Ok(Self { start, end })
    }
}
#[derive(Debug, Clone, Copy, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SplitMode {
    Copy,
    Accurate,
}
impl SplitMode {
    pub fn extension(self) -> &'static str {
        match self {
            Self::Copy => "mkv",
            Self::Accurate => "mp4",
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct StreamDisposition {
    #[serde(default)]
    pub attached_pic: u32,
}
impl MediaInfo {
    /// Matches FFmpeg's V selector: exclude album artwork and attached pictures.
    pub fn has_video(&self) -> bool {
        self.streams.iter().any(|s| {
            s.codec_type.as_deref() == Some("video")
                && s.disposition.as_ref().is_none_or(|d| d.attached_pic == 0)
        })
    }
}

#[derive(Debug, Deserialize)]
pub struct StreamSideData {
    pub rotation: Option<f64>,
}

#[derive(Debug, Deserialize)]
pub struct StreamTags {
    pub language: Option<String>,
    pub title: Option<String>,
}
