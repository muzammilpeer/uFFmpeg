use crate::{MediaInfo, Stream};
use std::collections::HashSet;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct StreamSelection {
    pub video: Option<u32>,
    pub audio: Vec<u32>,
}
impl StreamSelection {
    pub fn for_source(info: &MediaInfo) -> Self {
        Self {
            video: info.first_video().map(|s| s.index),
            audio: info
                .streams
                .iter()
                .filter(|s| s.codec_type.as_deref() == Some("audio"))
                .map(|s| s.index)
                .collect(),
        }
    }
    pub fn validate(&self, info: &MediaInfo) -> Result<(), String> {
        if let Some(index) = self.video
            && !info.streams.iter().any(|s| {
                s.index == index
                    && s.codec_type.as_deref() == Some("video")
                    && s.disposition.as_ref().is_none_or(|d| d.attached_pic == 0)
            })
        {
            return Err(format!(
                "Stream #{index} is not a moving video stream in this source. Re-select a video track."
            ));
        }
        let mut unique = HashSet::new();
        for index in &self.audio {
            if !unique.insert(*index) {
                return Err(format!(
                    "Audio stream #{index} was selected more than once."
                ));
            }
            if !info
                .streams
                .iter()
                .any(|s| s.index == *index && s.codec_type.as_deref() == Some("audio"))
            {
                return Err(format!(
                    "Stream #{index} is not an audio stream in this source. Re-select the audio tracks."
                ));
            }
        }
        Ok(())
    }
}
#[derive(Debug, Clone, Copy, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AudioFormat {
    Copy,
    Aac,
    Wav,
}
impl AudioFormat {
    pub fn extension(self) -> &'static str {
        match self {
            Self::Copy => "mka",
            Self::Aac => "m4a",
            Self::Wav => "wav",
        }
    }
    pub fn validate_codec(self, stream: &Stream) -> Result<(), String> {
        if matches!(self, Self::Copy)
            && !matches!(
                stream.codec_name.as_deref(),
                Some(
                    "aac"
                        | "mp3"
                        | "mp2"
                        | "opus"
                        | "vorbis"
                        | "flac"
                        | "ac3"
                        | "eac3"
                        | "dts"
                        | "truehd"
                        | "alac"
                        | "pcm_s16le"
                        | "pcm_s24le"
                        | "pcm_s32le"
                        | "pcm_f32le"
                        | "pcm_f64le"
                        | "pcm_u8"
                )
            )
        {
            return Err(format!(
                "Stream #{} uses {}, which is not approved for MKA copy. Choose AAC or WAV conversion instead.",
                stream.index,
                stream.codec_name.as_deref().unwrap_or("an unknown codec")
            ));
        }
        Ok(())
    }
}
impl Stream {
    pub fn label(&self) -> String {
        let mut text = format!(
            "#{} · {} · {}",
            self.index,
            self.codec_type.as_deref().unwrap_or("unknown"),
            self.codec_name.as_deref().unwrap_or("unknown")
        );
        if let (Some(w), Some(h)) = (self.width, self.height) {
            text.push_str(&format!(" · {w}×{h}"));
        }
        if let Some(rate) = &self.sample_rate {
            text.push_str(&format!(" · {rate} Hz"));
        }
        if let Some(channels) = self.channels {
            text.push_str(&format!(" · {channels} ch"));
        }
        if let Some(tags) = &self.tags {
            if let Some(language) = &tags.language {
                text.push_str(&format!(" · {language}"));
            }
            if let Some(title) = &tags.title {
                text.push_str(&format!(" · {title}"));
            }
        }
        text
    }
}
