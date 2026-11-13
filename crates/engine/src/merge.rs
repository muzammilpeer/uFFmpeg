use super::{Toolchain, command, control::run_process};
use frameforge_domain::{MediaInfo, MergeTarget};
use std::{
    io::Write,
    path::{Path, PathBuf},
    time::Duration,
};

impl Toolchain {
    /// Only application-owned staging paths may be passed as output.
    pub fn concat_merge(
        &self,
        inputs: &[PathBuf],
        output: &Path,
        normalized: bool,
    ) -> Result<(), String> {
        let manifest_directory = if normalized {
            Some(
                inputs
                    .first()
                    .and_then(|p| p.parent())
                    .ok_or("No normalized inputs")?,
            )
        } else {
            None
        };
        let mut builder = tempfile::Builder::new();
        builder.suffix(".ffconcat");
        let mut manifest = match manifest_directory {
            Some(directory) => builder.tempfile_in(directory),
            None => builder.tempfile(),
        }
        .map_err(|e| e.to_string())?;
        writeln!(manifest, "ffconcat version 1.0").map_err(|e| e.to_string())?;
        for input in inputs {
            let input = input.canonicalize().map_err(|e| e.to_string())?;
            let url = if let Some(directory) = manifest_directory {
                if input.parent() != Some(directory) {
                    return Err("Normalized inputs must share a workspace.".into());
                }
                input
                    .file_name()
                    .and_then(|s| s.to_str())
                    .ok_or("Invalid normalized clip name")?
                    .to_string()
            } else {
                let path = input.to_str().ok_or("Fast copy cannot represent this filename. Rename the file or use normalization.")?;
                #[cfg(windows)]
                let path = windows_concat_path(path);
                if path.contains(['\n', '\r']) {
                    return Err("Fast copy cannot represent a filename with line breaks. Rename it or choose normalization.".into());
                }
                format!("file:{path}")
            };
            // FFconcat single quotes protect spaces/backslashes; apostrophes are escaped outside them.
            let quoted = url.replace('\'', "'\\''");
            writeln!(manifest, "file '{quoted}'").map_err(|e| e.to_string())?;
        }
        manifest.flush().map_err(|e| e.to_string())?;
        let mut process = command(&self.ffmpeg);
        process
            .args([
                "-hide_banner",
                "-loglevel",
                "error",
                "-nostdin",
                "-y",
                "-f",
                "concat",
                "-safe",
                "0",
                "-protocol_whitelist",
                "file",
                "-i",
            ])
            .arg(manifest.path())
            .args([
                "-map",
                "0:V:0",
                "-map",
                "0:a?",
                "-map_metadata",
                "-1",
                "-map_chapters",
                "-1",
            ]);
        if normalized {
            process.args([
                "-c:v",
                "mpeg4",
                "-q:v",
                "3",
                "-pix_fmt",
                "yuv420p",
                "-r",
                "30",
                "-fps_mode",
                "cfr",
                "-c:a",
                "aac",
                "-b:a",
                "192k",
                "-movflags",
                "+faststart",
                "-f",
                "mp4",
            ]);
        } else {
            process.args(["-c", "copy", "-f", "matroska"]);
        }
        process.arg(output);
        run_process(
            &mut process,
            Duration::from_secs(3600),
            self.control(),
            true,
            self.control().progress().duration,
        )?;
        Ok(())
    }

    /// Lossless intermediates avoid a second lossy generation and keep large input lists off the command line.
    pub fn normalize_merge_clip(
        &self,
        input: &Path,
        info: &MediaInfo,
        output: &Path,
        target: MergeTarget,
        duration: f64,
    ) -> Result<(), String> {
        let mut process = command(&self.ffmpeg);
        process
            .args(["-hide_banner", "-loglevel", "error", "-nostdin", "-y", "-i"])
            .arg(input);
        let duration = format!("{:.9}", (duration * 30.0).ceil() / 30.0);
        // Input timestamps share a container-relative origin. Do not independently
        // reset video STARTPTS: that would shift video relative to delayed audio.
        // fps start_time=0 fills an initial video gap while retaining stream timing.
        let mut graph = format!(
            "[0:V:0]scale={}:{}:force_original_aspect_ratio=decrease:force_divisible_by=2:reset_sar=1,pad={}:{}:(ow-iw)/2:(oh-ih)/2,setsar=1,fps=fps=30:start_time=0,tpad=stop_mode=clone:stop_duration={duration},trim=duration={duration},setpts=PTS-STARTPTS[v]",
            target.width, target.height, target.width, target.height
        );
        for track in 0..target.audio_tracks {
            if track < info.audio_count() {
                graph.push_str(&format!(";[0:a:{track}]aresample=48000:async=1:first_pts=0,aformat=sample_fmts=s16:sample_rates=48000:channel_layouts=stereo,apad,atrim=duration={duration},asetpts=PTS-STARTPTS[a{track}]"));
            } else {
                graph.push_str(&format!(";anullsrc=r=48000:cl=stereo,atrim=duration={duration},asetpts=PTS-STARTPTS[a{track}]"));
            }
        }
        process.args(["-filter_complex", &graph, "-map", "[v]"]);
        for track in 0..target.audio_tracks {
            process.arg("-map").arg(format!("[a{track}]"));
        }
        process
            .args([
                "-map_metadata",
                "-1",
                "-map_chapters",
                "-1",
                "-c:v",
                "ffv1",
                "-level",
                "3",
                "-pix_fmt",
                "yuv420p",
                "-c:a",
                "pcm_s16le",
                "-f",
                "matroska",
            ])
            .arg(output);
        run_process(
            &mut process,
            Duration::from_secs(3600),
            self.control(),
            true,
            duration.parse::<f64>().ok(),
        )?;
        Ok(())
    }
}

#[cfg(windows)]
fn windows_concat_path(path: &str) -> String {
    if let Some(unc) = path.strip_prefix(r"\\?\UNC\") {
        format!("//{}", unc.replace('\\', "/"))
    } else {
        path.strip_prefix(r"\\?\")
            .unwrap_or(path)
            .replace('\\', "/")
    }
}
