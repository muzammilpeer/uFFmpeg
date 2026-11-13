use crate::{AppWindow, JobRow, Library};
use frameforge_application::jobs::{JobQueue, JobRequest, JobState, QueueHandle};
use slint::{ComponentHandle, ModelRc, VecModel};
use std::{collections::HashSet, path::PathBuf};

pub fn default_history() -> PathBuf {
    if let Some(dir) = std::env::var_os("UFFMPEG_STATE_DIR") {
        return PathBuf::from(dir).join("jobs.json");
    }
    let home = std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(std::env::temp_dir);
    #[cfg(target_os = "macos")]
    {
        home.join("Library/Application Support/uFFmpeg/jobs.json")
    }
    #[cfg(windows)]
    {
        std::env::var_os("LOCALAPPDATA")
            .map(PathBuf::from)
            .unwrap_or(home)
            .join("uFFmpeg/jobs.json")
    }
    #[cfg(all(not(windows), not(target_os = "macos")))]
    {
        std::env::var_os("XDG_DATA_HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|| home.join(".local/share"))
            .join("uFFmpeg/jobs.json")
    }
}
pub fn open_queue() -> (JobQueue, String) {
    match JobQueue::open(&default_history()) {
        Ok(queue) => (queue, String::new()),
        Err(error) => (JobQueue::new(), format!("Session-only queue: {error}")),
    }
}
pub fn submit(ui: &AppWindow, queue: &QueueHandle, request: JobRequest) {
    match queue.submit(request) {
        Ok(id) => {
            ui.set_selected_job_id(id.to_string().into());
            ui.set_import_status(format!("Queued job #{id}. Open Jobs to follow progress or cancel; you can keep preparing other jobs.").into());
        }
        Err(e) => ui.set_import_status(e.into()),
    }
}
pub fn connect(
    ui: &AppWindow,
    library: Library,
    queue: QueueHandle,
    initial_warning: String,
) -> slint::Timer {
    let (weak, handle) = (ui.as_weak(), queue.clone());
    ui.on_cancel_job(move |id|{if let Ok(id)=id.parse::<u64>(){handle.cancel(id);}
        if let Some(ui)=weak.upgrade(){ui.set_import_status("Cancellation requested. Running tools will stop before temporary files are removed.".into());}});
    let (weak, handle) = (ui.as_weak(), queue.clone());
    ui.on_retry_job(move |id| {
        if let (Some(ui), Ok(id)) = (weak.upgrade(), id.parse::<u64>()) {
            match handle.retry(id) {
                Ok(new_id) => {
                    ui.set_selected_job_id(new_id.to_string().into());
                    ui.set_import_status(
                        format!(
                            "Queued retry #{new_id}. Existing completed files remain protected."
                        )
                        .into(),
                    );
                }
                Err(e) => ui.set_import_status(e.into()),
            }
        }
    });
    let weak = ui.as_weak();
    let timer = slint::Timer::default();
    let mut previous_rows = Vec::new();
    timer.start(
        slint::TimerMode::Repeated,
        std::time::Duration::from_millis(150),
        move || {
            let Some(ui) = weak.upgrade() else {
                return;
            };
            let imported = queue.take_imports();
            if !imported.is_empty() {
                let mut items = library.borrow_mut();
                let mut known: HashSet<_> = items.iter().map(|i| i.path.clone()).collect();
                let first = items.len() as i32;
                for item in imported {
                    if known.insert(item.path.clone()) {
                        items.push(item);
                    }
                }
                let selected = if ui.get_selected() < 0 && items.len() as i32 > first {
                    first
                } else {
                    ui.get_selected()
                };
                let draft_start = ui.get_range_start();
                let draft_end = ui.get_range_end();
                let keep_draft = ui.get_selected() >= 0;
                crate::refresh(&ui, &items, selected);
                if keep_draft {
                    ui.set_range_start(draft_start);
                    ui.set_range_end(draft_end);
                }
            }
            let snapshots = queue.snapshots();
            let pending = snapshots.iter().filter(|j| !j.state.terminal()).count();
            ui.set_pending_jobs(pending as i32);
            let running = snapshots.iter().find(|j| j.state == JobState::Running);
            if let Some(job) = running {
                let progress = &job.progress;
                let percent = progress.percent();
                ui.set_active_progress(percent.unwrap_or(0.0) as f32);
                ui.set_active_job(
                    format!(
                        "#{} · {} · {} · {}/{} · {}{}",
                        job.id,
                        job.title,
                        progress.label,
                        progress.completed_units + 1,
                        progress.total_units.max(1),
                        percent
                            .map(|p| format!("{p:.0}%"))
                            .unwrap_or_else(|| format!("{:.1}s processed", progress.seconds)),
                        if progress.speed.is_empty() {
                            String::new()
                        } else {
                            format!(" · {}", progress.speed)
                        }
                    )
                    .into(),
                );
            } else {
                ui.set_active_progress(0.0);
                ui.set_active_job(if pending > 0 {
                    "Waiting for worker…".into()
                } else {
                    "Queue idle · You can prepare and queue exports from any source.".into()
                });
            }
            let selected = ui.get_selected_job_id().parse::<u64>().ok();
            if let Some(job) = snapshots.iter().find(|j| Some(j.id) == selected) {
                ui.set_job_details(
                    format!(
                        "#{} · {} · {}\n{}",
                        job.id,
                        job.title,
                        job.state.label(),
                        job.message
                    )
                    .into(),
                );
            }
            ui.set_can_reveal_output(
                selected.is_some_and(|id| !queue.completed_paths(id).is_empty()),
            );
            let rows = snapshots
                .iter()
                .filter(|job| !job.state.terminal())
                .chain(snapshots.iter().rev().filter(|job| job.state.terminal()))
                .map(|job| JobRow {
                    id: job.id.to_string().into(),
                    title: job.title.clone().into(),
                    state: job.state.label().into(),
                    can_cancel: !job.state.terminal(),
                    can_retry: matches!(
                        job.state,
                        JobState::Failed | JobState::Cancelled | JobState::Interrupted
                    ),
                })
                .collect::<Vec<_>>();
            // Preserve keyboard focus while only progress or elapsed time changes.
            if rows != previous_rows {
                previous_rows = rows.clone();
                ui.set_jobs(ModelRc::new(VecModel::from(rows)));
            }
            let warning = queue.warning();
            ui.set_queue_warning(if warning.is_empty() {
                initial_warning.clone().into()
            } else {
                warning.into()
            });
            if let Some(job) = snapshots
                .iter()
                .rev()
                .find(|j| j.title == "Media engine check")
            {
                ui.set_engine_status(if job.state.terminal() {
                    job.message.clone().into()
                } else {
                    "Checking media tools…".into()
                });
            }
            if ui.get_closing() && queue.is_idle() {
                let _ = ui.hide();
            }
        },
    );
    timer
}
