//! Native FFmpeg sidecar boundary. Never invokes a shell.
mod audio;
mod control;
pub use control::{ExecutionControl, Progress};
mod merge;
use frameforge_domain::{MediaInfo, SplitMode, StreamSelection, TimeRange};
use std::{
    env,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    time::Duration,
};
#[derive(Debug, Clone)]
pub struct Toolchain {
    pub ffmpeg: PathBuf,
    pub ffprobe: PathBuf,
    pub source: String,
    control: ExecutionControl,
}
fn executable(name: &str) -> String {
    format!("{name}{}", if cfg!(windows) { ".exe" } else { "" })
}
fn version(path: &Path, control: &ExecutionControl) -> Result<String, String> {
    let mut process = command(path);
    process.arg("-version");
    let output = control::run_process(&mut process, Duration::from_secs(10), control, false, None)?;
    Ok(String::from_utf8_lossy(&output)
        .lines()
        .next()
        .unwrap_or("Unknown version")
        .into())
}
fn command(path: &Path) -> Command {
    let mut command = Command::new(path);
    command.stdin(Stdio::null());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x08000000);
    }
    command
}
impl Toolchain {
    pub fn discover() -> Result<Self, String> {
        Self::discover_with_control(ExecutionControl::default())
    }
    pub fn control(&self) -> &ExecutionControl {
        &self.control
    }
    pub fn discover_with_control(control: ExecutionControl) -> Result<Self, String> {
        control.check()?;
        let exe = env::current_exe().map_err(|e| e.to_string())?;
        let parent = exe.parent().ok_or("Executable has no parent")?;
        let mut candidates = vec![
            (parent.join("../Resources/ffmpeg/bin"), "app resources"),
            (parent.join("ffmpeg/bin"), "installed sidecars"),
        ];
        // Production must never silently depend on a developer's PATH or override.
        if cfg!(debug_assertions) {
            if let Some(dir) = env::var_os("FRAMEFORGE_FFMPEG_DIR") {
                candidates.insert(0, (PathBuf::from(dir), "development override"));
            }
            if let Some(paths) = env::var_os("PATH") {
                candidates.extend(env::split_paths(&paths).map(|p| (p, "development PATH")));
            }
        }
        for (dir, source) in candidates {
            let chain = Self {
                ffmpeg: dir.join(executable("ffmpeg")),
                ffprobe: dir.join(executable("ffprobe")),
                source: source.into(),
                control: control.clone(),
            };
            if chain.ffmpeg.is_file() && chain.ffprobe.is_file() {
                chain.versions()?;
                return Ok(chain);
            }
        }
        Err("FFmpeg/FFprobe pair missing. Install bundled sidecars; debug builds also accept FRAMEFORGE_FFMPEG_DIR or PATH.".into())
    }
    pub fn versions(&self) -> Result<(String, String), String> {
        Ok((
            version(&self.ffmpeg, &self.control)?,
            version(&self.ffprobe, &self.control)?,
        ))
    }
    pub fn probe(&self, input: &Path) -> Result<MediaInfo, String> {
        let input = input.canonicalize().map_err(|e| e.to_string())?;
        let mut process = command(&self.ffprobe);
        process
            .args([
                "-v",
                "error",
                "-show_format",
                "-show_streams",
                "-show_data_hash",
                "sha256",
                "-of",
                "json",
            ])
            .arg(input);
        let output = control::run_process(
            &mut process,
            Duration::from_secs(20),
            &self.control,
            false,
            None,
        )?;
        serde_json::from_slice(&output).map_err(|e| format!("Invalid ffprobe metadata: {e}"))
    }
    /// Destination must be an application-owned staging file, never a user output path.
    pub fn split_segment(
        &self,
        input: &Path,
        output: &Path,
        range: TimeRange,
        mode: SplitMode,
        selection: &StreamSelection,
    ) -> Result<(), String> {
        let mut process = command(&self.ffmpeg);
        process
            .args([
                "-hide_banner",
                "-loglevel",
                "error",
                "-nostdin",
                "-y",
                "-ss",
            ])
            .arg(format!("{:.9}", range.start))
            .arg("-i")
            .arg(input)
            .arg("-t")
            .arg(format!("{:.9}", range.end - range.start));
        let video = selection
            .video
            .ok_or("Select a video stream before splitting.")?;
        process.arg("-map").arg(format!("0:{video}"));
        for audio in &selection.audio {
            process.arg("-map").arg(format!("0:{audio}"));
        }
        process.args(["-map_chapters", "-1"]);
        match mode {
            SplitMode::Copy => {
                process.args([
                    "-c",
                    "copy",
                    "-avoid_negative_ts",
                    "make_zero",
                    "-f",
                    "matroska",
                ]);
            }
            SplitMode::Accurate => {
                process.args([
                    "-c:v",
                    "mpeg4",
                    "-q:v",
                    "3",
                    "-vf",
                    "pad=ceil(iw/2)*2:ceil(ih/2)*2",
                    "-pix_fmt",
                    "yuv420p",
                    "-c:a",
                    "aac",
                    "-b:a",
                    "192k",
                    "-movflags",
                    "+faststart",
                    "-f",
                    "mp4",
                ]);
            }
        }
        process.arg(output);
        control::run_process(
            &mut process,
            Duration::from_secs(3600),
            &self.control,
            true,
            Some(range.end - range.start),
        )?;
        Ok(())
    }

    /// Generate one bounded-size preview; the destination is an application-owned cache file.
    pub fn thumbnail(&self, input: &Path, destination: &Path) -> Result<(), String> {
        let mut process = command(&self.ffmpeg);
        process
            .args(["-hide_banner", "-loglevel", "error", "-nostdin", "-y", "-i"])
            .arg(input)
            .args([
                "-map",
                "0:v:0",
                "-frames:v",
                "1",
                "-vf",
                "scale=320:180:force_original_aspect_ratio=decrease",
                "-threads",
                "1",
            ])
            .arg(destination);
        control::run_process(
            &mut process,
            Duration::from_secs(20),
            &self.control,
            true,
            None,
        )?;
        if !destination.is_file() {
            return Err("No video frame could be decoded".into());
        }
        Ok(())
    }
}
