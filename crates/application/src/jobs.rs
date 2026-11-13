//! One owned worker, immutable requests, coalesced progress and recoverable history.
use crate::{AudioFormat, ImportedMedia, MergeMode, SplitMode, StreamSelection, TimeRange};
use frameforge_engine::{ExecutionControl, Progress, Toolchain};
use serde::{Deserialize, Serialize};
use std::{
    path::{Path, PathBuf},
    sync::{Arc, Condvar, Mutex},
    thread::JoinHandle,
};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum JobRequest {
    Diagnostics,
    Import {
        paths: Vec<PathBuf>,
        #[serde(default)]
        existing: Vec<PathBuf>,
    },
    Split {
        input: PathBuf,
        directory: PathBuf,
        mode: SplitMode,
        #[serde(default)]
        ranges: Vec<TimeRange>,
        #[serde(default)]
        auto_seconds: Option<f64>,
        #[serde(default)]
        selection: Option<StreamSelection>,
    },
    Audio {
        input: PathBuf,
        directory: PathBuf,
        streams: Vec<u32>,
        format: AudioFormat,
        #[serde(default)]
        ranges: Option<Vec<TimeRange>>,
        #[serde(default)]
        auto_seconds: Option<f64>,
    },
    Merge {
        inputs: Vec<PathBuf>,
        destination: PathBuf,
        mode: MergeMode,
    },
}
impl JobRequest {
    fn absolutize(&mut self) -> Result<(), String> {
        let base = std::env::current_dir().map_err(|e| e.to_string())?;
        let absolute = |path: &mut PathBuf| {
            if path.is_relative() {
                *path = base.join(&*path);
            }
        };
        match self {
            Self::Diagnostics => (),
            Self::Import { paths, existing } => {
                for path in paths.iter_mut().chain(existing.iter_mut()) {
                    absolute(path);
                }
            }
            Self::Split {
                input, directory, ..
            }
            | Self::Audio {
                input, directory, ..
            } => {
                absolute(input);
                absolute(directory);
            }
            Self::Merge {
                inputs,
                destination,
                ..
            } => {
                for path in inputs {
                    absolute(path);
                }
                absolute(destination);
            }
        }
        Ok(())
    }
    pub fn title(&self) -> String {
        let (kind, path) = match self {
            Self::Diagnostics => return "Media engine check".into(),
            Self::Import { paths, .. } => return format!("Import {} source(s)", paths.len()),
            Self::Split { input, .. } => ("Split video", input),
            Self::Audio { input, .. } => ("Extract audio", input),
            Self::Merge { inputs, .. } => return format!("Merge {} clips", inputs.len()),
        };
        format!(
            "{kind} · {}",
            path.file_name().unwrap_or_default().to_string_lossy()
        )
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum JobState {
    Queued,
    Running,
    Completed,
    Failed,
    Cancelled,
    Interrupted,
}
impl JobState {
    pub fn terminal(self) -> bool {
        !matches!(self, Self::Queued | Self::Running)
    }
    pub fn label(self) -> &'static str {
        match self {
            Self::Queued => "Queued",
            Self::Running => "Running",
            Self::Completed => "Completed",
            Self::Failed => "Failed",
            Self::Cancelled => "Cancelled",
            Self::Interrupted => "Interrupted",
        }
    }
}
#[derive(Debug, Clone)]
pub struct JobSnapshot {
    pub id: u64,
    pub title: String,
    pub state: JobState,
    pub progress: Progress,
    pub message: String,
    pub completed_count: usize,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct History {
    id: u64,
    request: JobRequest,
    state: JobState,
    message: String,
    completed: Vec<PathBuf>,
}
struct Record {
    history: History,
    control: ExecutionControl,
    imported: Vec<ImportedMedia>,
}
struct State {
    records: Vec<Record>,
    next: u64,
    stopping: bool,
    warning: String,
    dirty: bool,
    journal_stop: bool,
}
struct Shared {
    state: Mutex<State>,
    wake: Condvar,
    journal_wake: Condvar,
    journal: Option<PathBuf>,
    _lock: Option<std::fs::File>,
}
#[derive(Clone)]
pub struct QueueHandle(Arc<Shared>);
pub struct JobQueue {
    handle: QueueHandle,
    worker: Option<JoinHandle<()>>,
    journal_worker: Option<JoinHandle<()>>,
}
fn bounded(mut text: String) -> String {
    if text.len() > 16 * 1024 {
        let mut end = 16 * 1024;
        while !text.is_char_boundary(end) {
            end -= 1;
        }
        text.truncate(end);
        text.push_str("\n[message truncated]");
    }
    text
}
fn save(shared: &Shared, state: &mut State) {
    if shared.journal.is_some() {
        state.dirty = true;
        shared.journal_wake.notify_one();
    }
}
fn journal_writer(shared: Arc<Shared>) {
    loop {
        let entries = {
            let mut state = shared.state.lock().unwrap_or_else(|e| e.into_inner());
            while !state.dirty && !state.journal_stop {
                state = shared
                    .journal_wake
                    .wait(state)
                    .unwrap_or_else(|e| e.into_inner());
            }
            if !state.dirty && state.journal_stop {
                return;
            }
            state.dirty = false;
            state
                .records
                .iter()
                .map(|r| r.history.clone())
                .collect::<Vec<_>>()
        };
        let result = (|| -> Result<(), String> {
            use std::io::Write;
            let path = shared.journal.as_ref().ok_or("History path unavailable")?;
            let bytes = serde_json::to_vec(&entries).map_err(|e| e.to_string())?;
            if bytes.len() > 64 * 1024 * 1024 {
                return Err("Queue history exceeded 64 MiB".into());
            }
            let mut temporary =
                tempfile::NamedTempFile::new_in(path.parent().ok_or("History has no parent")?)
                    .map_err(|e| e.to_string())?;
            temporary.write_all(&bytes).map_err(|e| e.to_string())?;
            temporary.as_file().sync_all().map_err(|e| e.to_string())?;
            temporary.persist(path).map_err(|e| e.to_string())?;
            Ok(())
        })();
        let mut state = shared.state.lock().unwrap_or_else(|e| e.into_inner());
        match result {
            Ok(()) => state.warning.clear(),
            Err(e) => {
                state.warning = format!(
                    "Queue history could not be saved: {e}. Current session jobs still work."
                )
            }
        }
    }
}
impl Default for JobQueue {
    fn default() -> Self {
        Self::new()
    }
}
impl JobQueue {
    pub fn new() -> Self {
        Self::start(None, None, Vec::new())
    }
    pub fn open(path: &Path) -> Result<Self, String> {
        let parent = path
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
            .unwrap_or(Path::new("."));
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        let path = parent
            .canonicalize()
            .map_err(|e| e.to_string())?
            .join(path.file_name().ok_or("Invalid history filename")?);
        let lock = std::fs::OpenOptions::new()
            .create(true)
            .truncate(false)
            .read(true)
            .write(true)
            .open(path.with_extension("lock"))
            .map_err(|e| e.to_string())?;
        lock.try_lock()
            .map_err(|e| format!("Another editor may be using this queue history: {e}"))?;
        let mut entries: Vec<History> = match std::fs::metadata(&path) {
            Ok(metadata) => {
                if metadata.len() > 64 * 1024 * 1024 {
                    return Err("Queue history is too large; it was left unchanged.".into());
                }
                serde_json::from_slice(&std::fs::read(&path).map_err(|e| e.to_string())?)
                    .map_err(|e| format!("Queue history is invalid and was left unchanged: {e}"))?
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Vec::new(),
            Err(e) => return Err(e.to_string()),
        };
        if entries.len() > 200 {
            return Err("Queue history contains too many records; it was left unchanged.".into());
        }
        let mut ids = std::collections::HashSet::new();
        for entry in &mut entries {
            if !ids.insert(entry.id) {
                return Err("Queue history has duplicate IDs; it was left unchanged.".into());
            }
            entry.message = bounded(std::mem::take(&mut entry.message));
            if !entry.state.terminal() {
                entry.state = JobState::Interrupted;
                entry.message = "Interrupted by an earlier exit. Review prior outputs before retrying. Existing files remain protected; prepare a new export to change its destination. Nothing was restarted automatically.".into();
            }
        }
        Ok(Self::start(Some(path), Some(lock), entries))
    }
    fn start(journal: Option<PathBuf>, lock: Option<std::fs::File>, entries: Vec<History>) -> Self {
        let next = entries
            .iter()
            .map(|r| r.id)
            .max()
            .unwrap_or(0)
            .saturating_add(1);
        let records = entries
            .into_iter()
            .map(|history| Record {
                history,
                control: ExecutionControl::default(),
                imported: Vec::new(),
            })
            .collect();
        let shared = Arc::new(Shared {
            state: Mutex::new(State {
                records,
                next,
                stopping: false,
                warning: String::new(),
                dirty: false,
                journal_stop: false,
            }),
            wake: Condvar::new(),
            journal_wake: Condvar::new(),
            journal,
            _lock: lock,
        });
        {
            let mut state = shared.state.lock().unwrap_or_else(|e| e.into_inner());
            save(&shared, &mut state);
        }
        let journal_worker = if shared.journal.is_some() {
            let writer_shared = shared.clone();
            Some(std::thread::spawn(move || journal_writer(writer_shared)))
        } else {
            None
        };
        let worker_shared = shared.clone();
        let worker = std::thread::spawn(move || worker(worker_shared));
        Self {
            handle: QueueHandle(shared),
            worker: Some(worker),
            journal_worker,
        }
    }
    pub fn handle(&self) -> QueueHandle {
        self.handle.clone()
    }
    pub fn shutdown(&mut self) {
        self.handle.request_shutdown();
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
        {
            let mut state = self
                .handle
                .0
                .state
                .lock()
                .unwrap_or_else(|e| e.into_inner());
            state.journal_stop = true;
            self.handle.0.journal_wake.notify_one();
        }
        if let Some(writer) = self.journal_worker.take() {
            let _ = writer.join();
        }
    }
}
impl Drop for JobQueue {
    fn drop(&mut self) {
        self.shutdown();
    }
}
impl QueueHandle {
    pub fn submit(&self, mut request: JobRequest) -> Result<u64, String> {
        request.absolutize()?;
        if serde_json::to_vec(&request)
            .map_err(|e| e.to_string())?
            .len()
            > 2 * 1024 * 1024
        {
            return Err("This job request exceeds 2 MiB. Use a smaller batch.".into());
        }
        if let JobRequest::Import { paths, .. } = &request
            && paths.len() > 1000
        {
            return Err("Import at most 1000 sources per job.".into());
        }
        let mut state = self.0.state.lock().unwrap_or_else(|e| e.into_inner());
        if state.stopping {
            return Err("The queue is shutting down.".into());
        }
        if state
            .records
            .iter()
            .filter(|r| !r.history.state.terminal())
            .count()
            >= 100
        {
            return Err("The queue already has 100 pending jobs. Wait for some to finish.".into());
        }
        if state.records.len() >= 200
            && let Some(index) = state
                .records
                .iter()
                .position(|r| r.history.state.terminal())
        {
            state.records.remove(index);
        }
        let id = state.next;
        state.next = state.next.checked_add(1).ok_or("Queue ID limit reached")?;
        let control = ExecutionControl::default();
        let weak = Arc::downgrade(&self.0);
        control.on_commit(move |path| {
            if let Some(shared) = weak.upgrade() {
                let mut state = shared.state.lock().unwrap_or_else(|e| e.into_inner());
                if let Some(record) = state.records.iter_mut().find(|r| r.history.id == id) {
                    record.history.completed.push(path.to_path_buf());
                }
                save(&shared, &mut state);
            }
        });
        state.records.push(Record {
            history: History {
                id,
                request,
                state: JobState::Queued,
                message: "Waiting for an available worker".into(),
                completed: Vec::new(),
            },
            control,
            imported: Vec::new(),
        });
        save(&self.0, &mut state);
        drop(state);
        self.0.wake.notify_one();
        Ok(id)
    }
    pub fn snapshots(&self) -> Vec<JobSnapshot> {
        self.0
            .state
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .records
            .iter()
            .map(|r| JobSnapshot {
                id: r.history.id,
                title: r.history.request.title(),
                state: r.history.state,
                progress: r.control.progress(),
                message: r.history.message.clone(),
                completed_count: r.history.completed.len(),
            })
            .collect()
    }
    pub fn warning(&self) -> String {
        self.0
            .state
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .warning
            .clone()
    }
    pub fn completed_paths(&self, id: u64) -> Vec<PathBuf> {
        self.0
            .state
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .records
            .iter()
            .find(|r| r.history.id == id)
            .map(|r| r.history.completed.clone())
            .unwrap_or_default()
    }
    pub fn take_imports(&self) -> Vec<ImportedMedia> {
        let mut state = self.0.state.lock().unwrap_or_else(|e| e.into_inner());
        state
            .records
            .iter_mut()
            .flat_map(|r| std::mem::take(&mut r.imported))
            .collect()
    }
    pub fn is_idle(&self) -> bool {
        self.0
            .state
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .records
            .iter()
            .all(|r| r.history.state.terminal())
    }
    pub fn cancel(&self, id: u64) {
        // Never hold the state mutex while waiting for the publication gate.
        let control = {
            let state = self.0.state.lock().unwrap_or_else(|e| e.into_inner());
            state
                .records
                .iter()
                .find(|r| r.history.id == id && !r.history.state.terminal())
                .map(|r| r.control.clone())
        };
        if let Some(control) = control {
            control.cancel();
            let mut state = self.0.state.lock().unwrap_or_else(|e| e.into_inner());
            if let Some(r) = state.records.iter_mut().find(|r| r.history.id == id)
                && r.history.state == JobState::Queued
            {
                r.history.state = JobState::Cancelled;
                r.history.message = "Cancelled before starting; no files were written.".into();
            }
            save(&self.0, &mut state);
            self.0.wake.notify_one();
        }
    }
    pub fn request_shutdown(&self) {
        let ids = {
            let mut state = self.0.state.lock().unwrap_or_else(|e| e.into_inner());
            state.stopping = true;
            state
                .records
                .iter()
                .filter(|r| !r.history.state.terminal())
                .map(|r| r.history.id)
                .collect::<Vec<_>>()
        };
        for id in ids {
            self.cancel(id);
        }
        self.0.wake.notify_one();
    }
    pub fn retry(&self, id: u64) -> Result<u64, String> {
        let request = {
            let state = self.0.state.lock().unwrap_or_else(|e| e.into_inner());
            let record = state
                .records
                .iter()
                .find(|r| r.history.id == id)
                .ok_or("Job not found")?;
            if !matches!(
                record.history.state,
                JobState::Failed | JobState::Cancelled | JobState::Interrupted
            ) {
                return Err("Only failed, cancelled or interrupted jobs can be retried.".into());
            }
            record.history.request.clone()
        };
        self.submit(request)
    }
}
struct Output {
    message: String,
    error: bool,
    imported: Vec<ImportedMedia>,
}
fn execute(request: JobRequest, control: ExecutionControl) -> Result<Output, String> {
    control.phase(0, 1, "Finding media tools", None);
    let tools = Toolchain::discover_with_control(control.clone())?;
    let mut imported = Vec::new();
    let (message, error) = match request {
        JobRequest::Diagnostics => {
            let (a, b) = tools.versions()?;
            (format!("Ready · {}\n{a}\n{b}", tools.source), false)
        }
        JobRequest::Import { paths, existing } => {
            let mut errors = String::new();
            let mut known = existing;
            let total = paths.len();
            for (index, path) in paths.into_iter().enumerate() {
                control.check()?;
                control.phase(
                    index,
                    total,
                    format!("Importing source {} of {total}", index + 1),
                    None,
                );
                if let Ok(path) = path.canonicalize()
                    && known.contains(&path)
                {
                    continue;
                }
                match crate::import_media(&tools, &path) {
                    Ok(item) => {
                        known.push(item.path.clone());
                        imported.push(item);
                    }
                    Err(e) => {
                        if errors.len() < 16 * 1024 {
                            errors.push_str(&format!("\n{}: {e}", path.display()));
                        }
                    }
                }
            }
            (
                format!("Imported {} source(s).{errors}", imported.len()),
                !errors.is_empty(),
            )
        }
        JobRequest::Split {
            input,
            directory,
            mode,
            ranges,
            auto_seconds,
            selection,
        } => {
            let report = match auto_seconds {
                Some(seconds) => crate::auto_split_video(
                    &tools,
                    &input,
                    &directory,
                    seconds,
                    mode,
                    selection.as_ref(),
                ),
                None => crate::split_media_selected(
                    &tools,
                    &input,
                    &directory,
                    &ranges,
                    mode,
                    selection.as_ref(),
                ),
            };
            (report.message(), report.error.is_some())
        }
        JobRequest::Audio {
            input,
            directory,
            streams,
            format,
            ranges,
            auto_seconds,
        } => {
            let report = match auto_seconds {
                Some(seconds) => {
                    crate::auto_extract_audio(&tools, &input, &directory, &streams, format, seconds)
                }
                None => crate::extract_audio(
                    &tools,
                    &input,
                    &directory,
                    &streams,
                    format,
                    ranges.as_deref(),
                ),
            };
            (report.message(), report.error.is_some())
        }
        JobRequest::Merge {
            inputs,
            destination,
            mode,
        } => {
            let path = crate::merge_media(&tools, &inputs, &destination, mode)?;
            (format!("Saved {}", path.display()), false)
        }
    };
    control.check()?;
    Ok(Output {
        message: bounded(message),
        error,
        imported,
    })
}
fn worker(shared: Arc<Shared>) {
    loop {
        let (id, request, control) = {
            let mut state = shared.state.lock().unwrap_or_else(|e| e.into_inner());
            loop {
                if state.stopping {
                    return;
                }
                if let Some(record) = state
                    .records
                    .iter_mut()
                    .find(|r| r.history.state == JobState::Queued)
                {
                    record.history.state = JobState::Running;
                    record.history.message = "Starting…".into();
                    let job = (
                        record.history.id,
                        record.history.request.clone(),
                        record.control.clone(),
                    );
                    save(&shared, &mut state);
                    break job;
                }
                state = shared.wake.wait(state).unwrap_or_else(|e| e.into_inner());
            }
        };
        let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            execute(request, control.clone())
        }));
        let mut state = shared.state.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(record) = state.records.iter_mut().find(|r| r.history.id == id) {
            let (message,error,imported)=match outcome {Ok(Ok(output))=>(output.message,output.error,output.imported),Ok(Err(e))=>(e,true,Vec::new()),Err(_)=>("Worker operation panicked; child cleanup completed before the queue continued.".into(),true,Vec::new())};
            record.history.state = if control.is_cancelled() {
                JobState::Cancelled
            } else if error {
                JobState::Failed
            } else {
                JobState::Completed
            };
            record.history.message = if matches!(
                record.history.request,
                JobRequest::Diagnostics | JobRequest::Import { .. }
            ) {
                bounded(message)
            } else {
                let mut message = format!(
                    "{message}\n{} completed output file(s) retained.",
                    record.history.completed.len()
                );
                if control.is_cancelled() {
                    for path in &record.history.completed {
                        if message.len() >= 16 * 1024 {
                            break;
                        }
                        message.push_str(&format!("\n{}", path.display()));
                    }
                }
                bounded(message)
            };
            record.imported = imported;
        }
        save(&shared, &mut state);
        shared.wake.notify_all();
    }
}
