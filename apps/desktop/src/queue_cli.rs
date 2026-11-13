use frameforge_application::jobs::{JobQueue, JobRequest, JobState};
use std::{
    collections::HashMap,
    io::BufRead,
    path::{Path, PathBuf},
    time::Duration,
};
pub fn run(manifest: &Path, history: Option<PathBuf>) -> Result<(), Box<dyn std::error::Error>> {
    if std::fs::metadata(manifest)?.len() > 32 * 1024 * 1024 {
        return Err("Queue manifest exceeds 32 MiB".into());
    }
    let requests: Vec<JobRequest> = serde_json::from_slice(&std::fs::read(manifest)?)?;
    let mut queue =
        JobQueue::open(&history.unwrap_or_else(|| manifest.with_extension("history.json")))?;
    let handle = queue.handle();
    let mut ids = Vec::new();
    for request in requests {
        ids.push(handle.submit(request)?);
    }
    println!(
        "Queued {} job(s). Commands on stdin: cancel ID, retry ID, quit.",
        ids.len()
    );
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        for line in std::io::stdin().lock().lines() {
            match line {
                Ok(line) => {
                    if tx.send(line).is_err() {
                        break;
                    }
                }
                Err(_) => break,
            }
        }
    });
    let mut seen = HashMap::new();
    loop {
        while let Ok(command) = rx.try_recv() {
            let fields = command.split_whitespace().collect::<Vec<_>>();
            match fields.as_slice() {
                ["cancel", id] => {
                    if let Ok(id) = id.parse::<u64>() {
                        handle.cancel(id);
                        println!("Cancellation requested for #{id}");
                    }
                }
                ["retry", id] => {
                    if let Ok(id) = id.parse::<u64>() {
                        match handle.retry(id) {
                            Ok(new) => {
                                ids.push(new);
                                println!("Retry queued as #{new}");
                            }
                            Err(e) => eprintln!("{e}"),
                        }
                    }
                }
                ["quit"] => {
                    handle.request_shutdown();
                    println!("Stopping and cleaning up…");
                }
                _ => eprintln!("Use cancel ID, retry ID or quit"),
            }
        }
        for job in handle
            .snapshots()
            .into_iter()
            .filter(|j| ids.contains(&j.id))
        {
            let percent = if job.state == JobState::Completed {
                Some(100)
            } else {
                job.progress.percent().map(|p| p as u32)
            };
            let status = format!(
                "#{} {} · {} · {}{}",
                job.id,
                job.state.label(),
                job.title,
                job.progress.label,
                percent.map(|p| format!(" · {p}%")).unwrap_or_default()
            );
            if seen.get(&job.id) != Some(&status) {
                println!("{status}");
                if job.state.terminal() {
                    println!("{}", job.message);
                }
                seen.insert(job.id, status);
            }
        }
        drop(handle.take_imports());
        if handle.is_idle() {
            break;
        }
        std::thread::sleep(Duration::from_millis(100));
    }
    queue.shutdown();
    let warning = handle.warning();
    if !warning.is_empty() {
        eprintln!("{warning}");
    }
    if handle
        .snapshots()
        .iter()
        .any(|j| ids.contains(&j.id) && j.state != JobState::Completed)
    {
        return Err("Some queued jobs did not complete; see their individual results".into());
    }
    Ok(())
}
