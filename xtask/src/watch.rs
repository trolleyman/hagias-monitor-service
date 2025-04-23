use std::{
    collections::HashSet,
    path::PathBuf,
    sync::{Arc, Mutex},
};

use anyhow::{Context as _, Result};
use watchexec::Watchexec;
use watchexec_signals::Signal;

#[derive(Debug)]
pub struct PathChangedFilterer;

impl watchexec::filter::Filterer for PathChangedFilterer {
    fn check_event(
        &self,
        event: &watchexec_events::Event,
        priority: watchexec_events::Priority,
    ) -> Result<bool, watchexec::error::RuntimeError> {
        if priority == watchexec_events::Priority::Urgent {
            return Ok(true);
        }

        // If any tag is a Keyboard, Process, Signal, or ProcessCompletion, then return true
        if event.tags.iter().any(|tag| {
            matches!(
                tag,
                watchexec_events::Tag::Keyboard(_)
                    | watchexec_events::Tag::Process(_)
                    | watchexec_events::Tag::Signal(_)
                    | watchexec_events::Tag::ProcessCompletion(_)
            )
        }) {
            return Ok(true);
        }

        // If any tag is a Path, then check that it is a modify, create, or delete operation
        if event
            .tags
            .iter()
            .any(|tag| matches!(tag, watchexec_events::Tag::Path { .. }))
        {
            for tag in event.tags.iter() {
                if let watchexec_events::Tag::FileEventKind(file_event_kind) = tag {
                    if file_event_kind.is_create()
                        || file_event_kind.is_modify()
                        || file_event_kind.is_remove()
                    {
                        return Ok(true);
                    }
                }
            }
            return Ok(false);
        }
        Ok(true)
    }
}

pub fn run(_release: bool) -> Result<()> {
    let workspace_root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("..");
    let (files, directories) = crate::ignore::get_unignored_files_and_directories(&workspace_root)?;

    let rt = tokio::runtime::Runtime::new().context("failed to create tokio runtime")?;
    rt.block_on(async {
        let error = Arc::new(Mutex::new(None::<anyhow::Error>));
        let error_clone = error.clone();
        // let jobs_list =
        let wx = Watchexec::new(move |mut action| {
            // Get the files that changed, If they are not ignored in the .gitignore, then rebuild everything
            let changed_directories = set_global_error_return(
                error_clone.clone(),
                get_changed_paths_from_action(
                    action.events.clone(),
                    watchexec_events::FileType::Dir,
                ),
            )
            .unwrap_or_else(|| HashSet::new());
            let changed_files = set_global_error_return(
                error_clone.clone(),
                get_changed_paths_from_action(
                    action.events.clone(),
                    watchexec_events::FileType::File,
                ),
            )
            .unwrap_or_else(|| HashSet::new());

            let have_any_unignored_paths_changed =
                have_any_unignored_paths_changed(&files, &changed_files)
                    || have_any_unignored_paths_changed(&directories, &changed_directories);

            if have_any_unignored_paths_changed {
                // Kill all running builds
                // TODO

                // Create running builds again
                // action.create_job(command);
            }

            // If Ctrl-C is received, quit
            if action.signals().any(|sig| sig == Signal::Interrupt) {
                action.quit();
            }
            action
        })
        .context("failed to create watchexec")?;

        // Set the filterer
        wx.config.filterer(crate::watch::PathChangedFilterer);

        // Watch the current directory
        wx.config.pathset([workspace_root]);

        // Run watchexec
        wx.main()
            .await
            .context("failed to join watchexec")?
            .context("failed to run watchexec")?;

        Ok::<(), anyhow::Error>(())
    })?;

    // // Build the CSS
    // build_css(release)?;

    // // Build the binary
    // let cargo_path = env!("CARGO");
    // let args = if release {
    //     vec!["build", "--release"]
    // } else {
    //     vec!["build"]
    // };
    // run_command(cargo_path, args)?;

    // // Start service
    // let mut service_path = workspace_root.join("target");
    // if release {
    //     service_path = service_path.join("release");
    // } else {
    //     service_path = service_path.join("debug");
    // }
    // service_path = service_path.join("hagias-monitor-service.exe");
    // let mut child =
    //     run_command_background::<&Path, &str, [&str; 0]>(&service_path, [])?;
    Ok(())
}

fn have_any_unignored_paths_changed(
    unignored_paths: &HashSet<PathBuf>,
    changed_paths: &HashSet<PathBuf>,
) -> bool {
    changed_paths.intersection(unignored_paths).next().is_some()
}

fn set_global_error_return<T>(
    global_error: Arc<Mutex<Option<anyhow::Error>>>,
    result: Result<T>,
) -> Option<T> {
    match result {
        Ok(value) => Some(value),
        Err(error) => {
            set_global_error(global_error, error);
            None
        }
    }
}

fn set_global_error(global_error: Arc<Mutex<Option<anyhow::Error>>>, error: anyhow::Error) {
    *global_error.lock().expect("failed to lock global error") = Some(error);
}

fn get_changed_paths_from_action(
    events: Arc<[watchexec_events::Event]>,
    file_type: watchexec_events::FileType,
) -> Result<HashSet<PathBuf>> {
    events
        .iter()
        .flat_map(|event| {
            event.tags.iter().filter_map(|tag| {
                if let watchexec_events::Tag::Path {
                    path,
                    file_type: matched_file_type,
                } = tag
                {
                    if let Some(matched_file_type) = matched_file_type {
                        if *matched_file_type == file_type {
                            return Some(path);
                        }
                    } else {
                        return Some(path);
                    }
                }
                None
            })
        })
        .map(|path| {
            path.canonicalize()
                .with_context(|| format!("failed to canonicalize path: {}", path.display()))
        })
        .collect::<Result<HashSet<_>>>()
        .context("failed to get changed paths")
}
