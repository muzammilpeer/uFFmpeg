use super::{Toolchain, command, control::run_process};
use frameforge_domain::{AudioFormat, TimeRange};
use std::{path::Path, time::Duration};
impl Toolchain {
    /// Output is an application-owned staging path; one source audio stream per file.
    pub fn extract_audio_stream(
        &self,
        input: &Path,
        output: &Path,
        stream_index: u32,
        format: AudioFormat,
        range: Option<TimeRange>,
    ) -> Result<(), String> {
        let mut process = command(&self.ffmpeg);
        process.args(["-hide_banner", "-loglevel", "error", "-nostdin", "-y"]);
        if let Some(range) = range {
            process.arg("-ss").arg(format!("{:.9}", range.start));
        }
        process.arg("-i").arg(input);
        // Input seeking may land on an earlier video keyframe. For audio copy,
        // discard negative relative timestamps rather than keeping that preroll.
        if range.is_some() && matches!(format, AudioFormat::Copy) {
            process.args(["-ss", "0"]);
        }
        if let Some(range) = range {
            process
                .arg("-t")
                .arg(format!("{:.9}", range.end - range.start));
        }
        process.arg("-map").arg(format!("0:{stream_index}")).args([
            "-vn",
            "-sn",
            "-dn",
            "-map_metadata",
            "-1",
            "-map_chapters",
            "-1",
        ]);
        match format {
            AudioFormat::Copy => {
                process.args([
                    "-c:a",
                    "copy",
                    "-avoid_negative_ts",
                    "make_zero",
                    "-f",
                    "matroska",
                ]);
            }
            AudioFormat::Aac => {
                process.args([
                    "-af",
                    "asetpts=PTS-STARTPTS",
                    "-c:a",
                    "aac",
                    "-b:a",
                    "192k",
                    "-movflags",
                    "+faststart",
                    "-f",
                    "ipod",
                ]);
            }
            AudioFormat::Wav => {
                process.args([
                    "-af",
                    "asetpts=PTS-STARTPTS",
                    "-c:a",
                    "pcm_s16le",
                    "-rf64",
                    "auto",
                    "-f",
                    "wav",
                ]);
            }
        }
        process.arg(output);
        run_process(
            &mut process,
            Duration::from_secs(3600),
            self.control(),
            true,
            range
                .map(|r| r.end - r.start)
                .or_else(|| self.control().progress().duration),
        )?;
        Ok(())
    }
}
