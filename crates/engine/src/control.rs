use std::{
    path::Path,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
};

#[derive(Debug, Clone, Default)]
pub struct Progress {
    pub label: String,
    pub completed_units: usize,
    pub total_units: usize,
    pub seconds: f64,
    pub duration: Option<f64>,
    pub speed: String,
}
impl Progress {
    pub fn percent(&self) -> Option<f64> {
        let duration = self.duration.filter(|d| d.is_finite() && *d > 0.0)?;
        let fraction = (self.seconds / duration).clamp(0.0, 0.99);
        Some(
            ((self.completed_units as f64 + fraction) / self.total_units.max(1) as f64 * 100.0)
                .clamp(0.0, 99.9),
        )
    }
}
type CommitObserver = Arc<dyn Fn(&Path) + Send + Sync>;
#[derive(Default)]
struct State {
    cancelled: AtomicBool,
    progress: Mutex<Progress>,
    publication: Mutex<()>,
    observer: Mutex<Option<CommitObserver>>,
}
#[derive(Clone, Default)]
pub struct ExecutionControl(Arc<State>);
impl std::fmt::Debug for ExecutionControl {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ExecutionControl")
            .field("cancelled", &self.is_cancelled())
            .finish()
    }
}
impl ExecutionControl {
    pub fn cancel(&self) {
        let _gate = self.0.publication.lock().unwrap_or_else(|e| e.into_inner());
        self.0.cancelled.store(true, Ordering::SeqCst);
    }
    pub fn is_cancelled(&self) -> bool {
        self.0.cancelled.load(Ordering::SeqCst)
    }
    pub fn check(&self) -> Result<(), String> {
        if self.is_cancelled() {
            Err("Cancelled by user".into())
        } else {
            Ok(())
        }
    }
    pub fn progress(&self) -> Progress {
        self.0
            .progress
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone()
    }
    pub fn phase(
        &self,
        completed: usize,
        total: usize,
        label: impl Into<String>,
        duration: Option<f64>,
    ) {
        *self.0.progress.lock().unwrap_or_else(|e| e.into_inner()) = Progress {
            label: label.into(),
            completed_units: completed,
            total_units: total.max(1),
            duration,
            ..Default::default()
        };
    }
    pub(crate) fn begin_command(&self, duration: Option<f64>) {
        let mut p = self.0.progress.lock().unwrap_or_else(|e| e.into_inner());
        p.seconds = 0.0;
        p.duration = duration;
        p.speed.clear();
    }
    pub(crate) fn progress_line(&self, line: &[u8]) {
        let Ok(line) = std::str::from_utf8(line) else {
            return;
        };
        let Some((key, value)) = line.trim().split_once('=') else {
            return;
        };
        let mut p = self.0.progress.lock().unwrap_or_else(|e| e.into_inner());
        if key == "out_time_us"
            && let Ok(us) = value.parse::<i64>()
        {
            p.seconds = p.seconds.max((us as f64 / 1_000_000.0).max(0.0));
        }
        if key == "speed" {
            p.speed = value.chars().take(32).collect();
        }
    }
    pub fn on_commit(&self, observer: impl Fn(&Path) + Send + Sync + 'static) {
        *self.0.observer.lock().unwrap_or_else(|e| e.into_inner()) = Some(Arc::new(observer));
    }
    /// Cancellation and publication are serialized: once cancel returns, no new file can commit.
    pub fn publish(
        &self,
        path: &Path,
        operation: impl FnOnce() -> Result<(), String>,
    ) -> Result<(), String> {
        let _gate = self.0.publication.lock().unwrap_or_else(|e| e.into_inner());
        self.check()?;
        operation()?;
        let observer = self
            .0
            .observer
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone();
        if let Some(observer) = observer {
            observer(path);
        }
        Ok(())
    }
}

/// A direct native child is always killed and reaped if its scope unwinds early.
struct ChildGuard {
    child: std::process::Child,
    done: bool,
}
impl Drop for ChildGuard {
    fn drop(&mut self) {
        if !self.done {
            let _ = self.child.kill();
            let _ = self.child.wait();
        }
    }
}
pub(crate) fn run_process(
    process: &mut std::process::Command,
    timeout: std::time::Duration,
    control: &ExecutionControl,
    media: bool,
    duration: Option<f64>,
) -> Result<Vec<u8>, String> {
    use std::{
        io::Read,
        process::Stdio,
        time::{Duration, Instant},
    };
    control.check()?;
    if media {
        process.args(["-progress", "pipe:1", "-nostats", "-stats_period", "0.2"]);
        control.begin_command(duration);
    }
    process.stdout(Stdio::piped()).stderr(Stdio::piped());
    let mut child = ChildGuard {
        child: process.spawn().map_err(|e| e.to_string())?,
        done: false,
    };
    let mut stdout = child.child.stdout.take().ok_or("No process stdout")?;
    let mut stderr = child.child.stderr.take().ok_or("No process stderr")?;
    let too_large = Arc::new(AtomicBool::new(false));
    let limit = too_large.clone();
    let progress = control.clone();
    let output = std::thread::spawn(move || -> Result<Vec<u8>, String> {
        let mut bytes = Vec::new();
        let mut line = Vec::new();
        let mut overflow = false;
        let mut buffer = [0u8; 4096];
        loop {
            let n = stdout.read(&mut buffer).map_err(|e| e.to_string())?;
            if n == 0 {
                break;
            }
            if media {
                for byte in &buffer[..n] {
                    if *byte == b'\n' {
                        if !overflow {
                            progress.progress_line(&line);
                        }
                        line.clear();
                        overflow = false;
                    } else if line.len() < 4096 {
                        line.push(*byte);
                    } else {
                        overflow = true;
                    }
                }
            } else if bytes.len() + n <= 4 * 1024 * 1024 {
                bytes.extend_from_slice(&buffer[..n]);
            } else {
                limit.store(true, Ordering::SeqCst);
            }
        }
        Ok(bytes)
    });
    let errors = std::thread::spawn(move || -> Result<Vec<u8>, String> {
        let mut tail = std::collections::VecDeque::with_capacity(16 * 1024);
        let mut buffer = [0u8; 4096];
        loop {
            let n = stderr.read(&mut buffer).map_err(|e| e.to_string())?;
            if n == 0 {
                break;
            }
            for byte in &buffer[..n] {
                if tail.len() == 16 * 1024 {
                    tail.pop_front();
                }
                tail.push_back(*byte);
            }
        }
        Ok(tail.into_iter().collect())
    });
    let start = Instant::now();
    let outcome = loop {
        let result = control.check().and_then(|()| {
            if too_large.load(Ordering::SeqCst) {
                return Err("Media inspection exceeded its 4 MiB output limit".into());
            }
            if start.elapsed() > timeout {
                return Err(format!(
                    "Media command timed out ({} seconds)",
                    timeout.as_secs()
                ));
            }
            child.child.try_wait().map_err(|e| e.to_string())
        });
        match result {
            Ok(Some(status)) => {
                child.done = true;
                break Ok(status);
            }
            Ok(None) => std::thread::sleep(Duration::from_millis(30)),
            Err(error) => {
                let _ = child.child.kill();
                let _ = child.child.wait();
                child.done = true;
                break Err(error);
            }
        }
    };
    let stdout = output.join().map_err(|_| "Process stdout reader stopped")?;
    let stderr = errors.join().map_err(|_| "Process stderr reader stopped")?;
    let status = outcome?;
    control.check()?;
    if too_large.load(Ordering::SeqCst) {
        return Err("Media inspection exceeded its 4 MiB output limit".into());
    }
    let stderr = stderr?;
    if !status.success() {
        return Err(format!(
            "Media tool failed ({status}): {}",
            String::from_utf8_lossy(&stderr)
        ));
    }
    stdout
}
