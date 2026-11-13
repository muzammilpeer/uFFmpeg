mod audio;
mod automatic;
mod jobs;
mod merge;
mod preferences;
mod queue_cli;
mod reveal;
mod split;
use frameforge_application::ImportedMedia;
use slint::{ModelRc, VecModel};
use std::{cell::RefCell, path::PathBuf, rc::Rc};
slint::include_modules!();
type Library = Rc<RefCell<Vec<ImportedMedia>>>;

fn refresh(ui: &AppWindow, library: &[ImportedMedia], selected: i32) {
    ui.set_media(ModelRc::new(VecModel::from(
        library
            .iter()
            .map(|item| MediaRow {
                name: item
                    .path
                    .file_name()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .into_owned()
                    .into(),
            })
            .collect::<Vec<_>>(),
    )));
    ui.set_selected(selected);
    split::refresh(ui, library, selected);
    merge::refresh(ui, library);
    audio::refresh(ui, library, selected);
    if let Some(item) = usize::try_from(selected).ok().and_then(|i| library.get(i)) {
        ui.set_selected_path(item.path.to_string_lossy().into_owned().into());
        ui.set_selected_details(item.summary.clone().into());
        let preview = item
            .thumbnail
            .as_deref()
            .map(slint::Image::load_from_path)
            .transpose();
        let note = match &preview {
            Err(e) => format!("Preview could not be loaded: {e}"),
            _ => item.warning.clone().unwrap_or_else(|| {
                if item.thumbnail.is_some() {
                    "Import preview: first video stream. Stream choices apply to exports.".into()
                } else {
                    "Audio-only media · no video preview".into()
                }
            }),
        };
        ui.set_preview(preview.ok().flatten().unwrap_or_default());
        ui.set_preview_note(note.into());
    } else {
        ui.set_selected_path("".into());
        ui.set_selected_details("Select a file to view media details.".into());
        ui.set_preview(Default::default());
        ui.set_preview_note("No media selected".into());
    }
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args_os().skip(1);
    if let Some(arg) = args.next() {
        if arg == "--run-queue" {
            let manifest = PathBuf::from(
                args.next()
                    .ok_or("Use --run-queue MANIFEST.json [--history HISTORY.json]")?,
            );
            let history = match args.next() {
                Some(flag) if flag == "--history" => {
                    Some(PathBuf::from(args.next().ok_or("History path required")?))
                }
                Some(_) => return Err("Expected --history PATH".into()),
                None => None,
            };
            if args.next().is_some() {
                return Err("Unexpected queue arguments".into());
            }
            return queue_cli::run(&manifest, history);
        }
        if arg == "--diagnostics" {
            println!(
                "{}",
                frameforge_application::diagnostics().map_err(std::io::Error::other)?
            );
            return Ok(());
        }
        if arg == "--plan-segments" {
            let duration = args
                .next()
                .ok_or("Source duration in seconds required")?
                .to_str()
                .ok_or("Duration must be numeric")?
                .parse::<f64>()?;
            let length = args.next().ok_or("Segment length required")?;
            let unit = args.next().ok_or("Choose seconds or minutes")?;
            if args.next().is_some() {
                return Err("Use --plan-segments DURATION_SECONDS LENGTH seconds|minutes".into());
            }
            let seconds = frameforge_application::parse_segment_seconds(
                length.to_str().ok_or("Segment length must be numeric")?,
                unit.to_str().ok_or("Invalid unit")?,
            )?;
            let ranges = frameforge_application::fixed_length_ranges(duration, seconds)?;
            println!("{} segment(s)", ranges.len());
            for (i, range) in ranges.iter().enumerate() {
                println!("{:03}: {:.6} → {:.6}", i + 1, range.start, range.end);
            }
            return Ok(());
        }
        if arg == "--auto-split" || arg == "--auto-split-streams" || arg == "--auto-extract-audio" {
            let input = PathBuf::from(args.next().ok_or("Source path required")?);
            let directory = PathBuf::from(args.next().ok_or("Output folder required")?);
            let mode = args.next().ok_or("Output mode required")?;
            let is_audio = arg == "--auto-extract-audio";
            let indices = if is_audio {
                parse_audio_indices(&args.next().ok_or("Audio stream indices required")?)?
            } else {
                Vec::new()
            };
            let selection = if arg == "--auto-split-streams" {
                let video = args
                    .next()
                    .ok_or("Video stream index required")?
                    .to_str()
                    .ok_or("Video index must be numeric")?
                    .parse::<u32>()?;
                let audio = parse_audio_indices(
                    &args.next().ok_or("Audio stream indices or none required")?,
                )?;
                Some(frameforge_application::StreamSelection {
                    video: Some(video),
                    audio,
                })
            } else {
                None
            };
            let length = args.next().ok_or("Segment length required")?;
            let unit = args.next().ok_or("Choose seconds or minutes")?;
            if args.next().is_some() {
                return Err("Unexpected arguments after segment length and unit".into());
            }
            let seconds = frameforge_application::parse_segment_seconds(
                length.to_str().ok_or("Segment length must be numeric")?,
                unit.to_str().ok_or("Invalid unit")?,
            )?;
            let tools = frameforge_engine::Toolchain::discover()?;
            if is_audio {
                let format = match mode.to_str() {
                    Some("copy") => frameforge_application::AudioFormat::Copy,
                    Some("aac") => frameforge_application::AudioFormat::Aac,
                    Some("wav") => frameforge_application::AudioFormat::Wav,
                    _ => return Err("Choose copy, aac or wav audio format".into()),
                };
                let report = frameforge_application::auto_extract_audio(
                    &tools, &input, &directory, &indices, format, seconds,
                );
                println!("{}", report.message());
                if report.error.is_some() {
                    return Err("Automatic audio splitting did not complete".into());
                }
            } else {
                let mode = match mode.to_str() {
                    Some("copy") => frameforge_application::SplitMode::Copy,
                    Some("accurate") => frameforge_application::SplitMode::Accurate,
                    _ => return Err("Choose copy or accurate video mode".into()),
                };
                let report = frameforge_application::auto_split_video(
                    &tools,
                    &input,
                    &directory,
                    seconds,
                    mode,
                    selection.as_ref(),
                );
                println!("{}", report.message());
                if report.error.is_some() {
                    return Err("Automatic video splitting did not complete".into());
                }
            }
            return Ok(());
        }
        if arg == "--streams" {
            let path = PathBuf::from(args.next().ok_or("Source media path required")?);
            let info = frameforge_engine::Toolchain::discover()?.probe(&path)?;
            for stream in info.streams {
                println!("{}", stream.label());
            }
            return Ok(());
        }
        if arg == "--extract-audio" {
            let input = PathBuf::from(args.next().ok_or(
                "--extract-audio INPUT FOLDER copy|aac|wav AUDIO_INDICES full|ranges [START END…]",
            )?);
            let directory = PathBuf::from(args.next().ok_or("Output folder required")?);
            let format = match args.next().and_then(|s| s.into_string().ok()).as_deref() {
                Some("copy") => frameforge_application::AudioFormat::Copy,
                Some("aac") => frameforge_application::AudioFormat::Aac,
                Some("wav") => frameforge_application::AudioFormat::Wav,
                _ => return Err("Choose copy, aac or wav audio format".into()),
            };
            let indices =
                parse_audio_indices(&args.next().ok_or("Audio stream indices required")?)?;
            let scope = args.next().ok_or("Choose full or ranges scope")?;
            let tools = frameforge_engine::Toolchain::discover()?;
            let ranges = if scope == "full" {
                if args.next().is_some() {
                    return Err("Full source extraction takes no range values".into());
                }
                None
            } else if scope == "ranges" {
                let duration = frameforge_application::media_duration(&tools.probe(&input)?)?;
                let values = args
                    .map(|s| {
                        s.into_string()
                            .map_err(|_| "Range must contain seconds")
                            .and_then(|s| s.parse::<f64>().map_err(|_| "Invalid numeric seconds"))
                    })
                    .collect::<Result<Vec<_>, _>>()?;
                if values.is_empty() || values.len() % 2 != 0 {
                    return Err("Supply START END pairs in seconds".into());
                }
                Some(
                    values
                        .chunks_exact(2)
                        .map(|v| frameforge_application::TimeRange::new(v[0], v[1], duration))
                        .collect::<Result<Vec<_>, _>>()?,
                )
            } else {
                return Err("Choose full or ranges scope".into());
            };
            let report = frameforge_application::extract_audio(
                &tools,
                &input,
                &directory,
                &indices,
                format,
                ranges.as_deref(),
            );
            println!("{}", report.message());
            if report.error.is_some() {
                return Err("Audio extraction did not complete".into());
            }
            return Ok(());
        }
        if arg == "--merge-check" {
            let inputs = args.map(PathBuf::from).collect::<Vec<_>>();
            let tools = frameforge_engine::Toolchain::discover()?;
            let overview = frameforge_application::inspect_merge(&tools, &inputs)?;
            println!("{}", overview.summary);
            if !overview.can_merge {
                return Err("These clips cannot be merged".into());
            }
            return Ok(());
        }
        if arg == "--merge" {
            let destination = PathBuf::from(
                args.next()
                    .ok_or("--merge needs OUTPUT copy|normalize INPUT INPUT [INPUT…]")?,
            );
            let mode = match args.next().and_then(|s| s.into_string().ok()).as_deref() {
                Some("copy") => frameforge_application::MergeMode::Copy,
                Some("normalize") => frameforge_application::MergeMode::Normalize,
                _ => return Err("Choose copy or normalize mode".into()),
            };
            let inputs = args.map(PathBuf::from).collect::<Vec<_>>();
            let tools = frameforge_engine::Toolchain::discover()?;
            let output = frameforge_application::merge_media(&tools, &inputs, &destination, mode)?;
            println!(
                "Merged {} whole clips. Saved {}",
                inputs.len(),
                output.display()
            );
            return Ok(());
        }
        if arg == "--split" || arg == "--split-streams" {
            let input = PathBuf::from(
                args.next()
                    .ok_or("--split requires INPUT FOLDER copy|accurate START END [START END…]")?,
            );
            let folder = PathBuf::from(args.next().ok_or("Output folder required")?);
            let mode = match args.next().and_then(|s| s.into_string().ok()).as_deref() {
                Some("copy") => frameforge_application::SplitMode::Copy,
                Some("accurate") => frameforge_application::SplitMode::Accurate,
                _ => return Err("Choose copy or accurate mode".into()),
            };
            let selection = if arg == "--split-streams" {
                let video = args
                    .next()
                    .ok_or("Video stream index required")?
                    .to_str()
                    .ok_or("Video index must be numeric")?
                    .parse::<u32>()?;
                let audio = parse_audio_indices(
                    &args.next().ok_or("Audio stream indices or none required")?,
                )?;
                Some(frameforge_application::StreamSelection {
                    video: Some(video),
                    audio,
                })
            } else {
                None
            };
            let tools = frameforge_engine::Toolchain::discover()?;
            let duration = frameforge_application::media_duration(&tools.probe(&input)?)?;
            let values = args
                .map(|s| {
                    s.into_string()
                        .map_err(|_| "Range must contain numeric seconds")
                        .and_then(|s| s.parse::<f64>().map_err(|_| "Invalid numeric seconds"))
                })
                .collect::<Result<Vec<_>, _>>()?;
            if values.is_empty() || values.len() % 2 != 0 {
                return Err("Supply START END pairs in seconds".into());
            }
            let ranges = values
                .chunks_exact(2)
                .map(|v| frameforge_application::TimeRange::new(v[0], v[1], duration))
                .collect::<Result<Vec<_>, _>>()?;
            let report = frameforge_application::split_media_selected(
                &tools,
                &input,
                &folder,
                &ranges,
                mode,
                selection.as_ref(),
            );
            println!("{}", report.message());
            if report.error.is_some() {
                return Err("Split export did not complete".into());
            }
            return Ok(());
        }
        if arg == "--probe" || arg == "--inspect" {
            let path = PathBuf::from(args.next().ok_or("Media path required")?);
            let tools = frameforge_engine::Toolchain::discover()?;
            if arg == "--inspect" {
                let item = frameforge_application::import_media(&tools, &path)?;
                println!(
                    "{}\n{}\nPreview generated: {}",
                    item.path.display(),
                    item.summary,
                    item.thumbnail.is_some()
                );
                if let Some(warning) = item.warning {
                    println!("{warning}");
                }
            } else {
                println!("{:#?}", tools.probe(&path)?);
            }
            return Ok(());
        }
        return Err("Supported arguments: --run-queue MANIFEST.json [--history HISTORY.json], --auto-split INPUT FOLDER copy|accurate LENGTH seconds|minutes, --auto-split-streams INPUT FOLDER copy|accurate VIDEO_INDEX AUDIO_INDICES|none LENGTH seconds|minutes, --auto-extract-audio INPUT FOLDER copy|aac|wav AUDIO_INDICES LENGTH seconds|minutes, --plan-segments DURATION_SECONDS LENGTH seconds|minutes, --streams INPUT, --extract-audio INPUT FOLDER copy|aac|wav AUDIO_INDICES full|ranges [START END…], --split-streams INPUT FOLDER copy|accurate VIDEO_INDEX AUDIO_INDICES|none START END [START END…], --merge OUTPUT copy|normalize INPUT INPUT [INPUT…], --merge-check INPUT INPUT [INPUT…], --diagnostics, --probe PATH, --inspect PATH, --split INPUT FOLDER copy|accurate START END [START END…]".into());
    }
    let ui = AppWindow::new()?;
    let library: Library = Default::default();
    let (mut job_queue, queue_warning) = jobs::open_queue();
    let queue = job_queue.handle();
    let _queue_timer = jobs::connect(&ui, library.clone(), queue.clone(), queue_warning);
    if let Err(error) = queue.submit(frameforge_application::jobs::JobRequest::Diagnostics) {
        ui.set_import_status(error.into());
    }
    split::connect(&ui, library.clone(), queue.clone());
    merge::connect(&ui, library.clone(), queue.clone());
    audio::connect(&ui, library.clone(), queue.clone());
    automatic::connect(&ui, library.clone());
    preferences::connect(&ui);
    reveal::connect(&ui, queue.clone());
    let close_weak = ui.as_weak();
    let close_queue = queue.clone();
    ui.window().on_close_requested(move || {
        close_queue.request_shutdown();
        if !close_queue.is_idle() {
            if let Some(ui) = close_weak.upgrade() {
                ui.set_closing(true);
                ui.set_busy(true);
                ui.set_import_status(
                    "Stopping media tools and cleaning temporary files before closing…".into(),
                );
            }
            return slint::CloseRequestResponse::KeepWindowShown;
        }
        slint::CloseRequestResponse::HideWindow
    });
    let (weak, items) = (ui.as_weak(), library.clone());
    ui.on_select_media(move |index| {
        if let Some(ui) = weak.upgrade() {
            if ui.get_busy() {
                return;
            }
            refresh(&ui, &items.borrow(), index);
        }
    });
    let (weak, items) = (ui.as_weak(), library.clone());
    ui.on_move_media(move |offset| {
        if let Some(ui) = weak.upgrade() {
            if ui.get_busy() {
                return;
            }
            let current = ui.get_selected();
            let next = current + offset;
            let mut items = items.borrow_mut();
            if current >= 0 && next >= 0 && (next as usize) < items.len() {
                items.swap(current as usize, next as usize);
                refresh(&ui, &items, next);
            }
        }
    });
    let (weak, items) = (ui.as_weak(), library.clone());
    ui.on_remove_media(move || {
        if let Some(ui) = weak.upgrade() {
            if ui.get_busy() {
                return;
            }
            let index = ui.get_selected();
            let mut items = items.borrow_mut();
            if index >= 0 && (index as usize) < items.len() {
                items.remove(index as usize);
                let selected = if items.is_empty() {
                    -1
                } else {
                    index.min(items.len() as i32 - 1)
                };
                refresh(&ui, &items, selected);
                ui.set_import_status("Removed from library. Source file was kept.".into());
            }
        }
    });
    let (weak, items) = (ui.as_weak(), library.clone());
    ui.on_import_files(move || {
        let Some(ui) = weak.upgrade() else {
            return;
        };
        if ui.get_busy() {
            return;
        }
        // macOS native panels must be initiated on the UI thread.
        let Some(paths) = preferences::dialog(false)
            .set_title("Add video or audio files")
            .pick_files()
        else {
            return;
        };
        if paths.is_empty() {
            return;
        }
        if let Some(folder) = paths.first().and_then(|p| p.parent()) {
            preferences::remember(&ui, folder, false);
        }
        let existing = items
            .borrow()
            .iter()
            .map(|item| item.path.clone())
            .collect();
        jobs::submit(
            &ui,
            &queue,
            frameforge_application::jobs::JobRequest::Import { paths, existing },
        );
    });
    ui.run()?;
    job_queue.shutdown();
    Ok(())
}

fn parse_audio_indices(value: &std::ffi::OsStr) -> Result<Vec<u32>, Box<dyn std::error::Error>> {
    let value = value
        .to_str()
        .ok_or("Audio stream indices must be numeric")?;
    if value == "none" {
        return Ok(Vec::new());
    }
    if value.is_empty() {
        return Err("Use comma-separated source stream indices, such as 1,2, or none.".into());
    }
    value
        .split(',')
        .map(|s| s.trim().parse::<u32>().map_err(Into::into))
        .collect()
}
