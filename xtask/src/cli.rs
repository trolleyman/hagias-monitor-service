use std::{
    collections::HashSet,
    path::PathBuf,
    sync::{Arc, Mutex},
};

use anyhow::{Context, Result};
use clap::Parser as _;
use watchexec::Watchexec;
use watchexec_signals::Signal;

use crate::{fs::normalize_path, print::print_cargo_style};

#[derive(clap::Parser)]
#[command(author, version, about, long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(clap::Subcommand)]
enum Commands {
    /// Package the binary into the bin folder
    Pack {
        /// Output directory for the packaged binary
        #[arg(default_value = "pack")]
        pack_dir: PathBuf,
    },
    /// Run the monitor service
    Run {
        /// Whether to run the release binary
        #[arg(short, long, default_value = "false")]
        release: bool,
    },
    /// Run the monitor service and watch for changes
    Watch {
        /// Whether to run the release binary
        #[arg(short, long, default_value = "false")]
        release: bool,
    },
}

pub fn run() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Pack { pack_dir } => {
            // Create target directory if it doesn't exist
            print_cargo_style(
                "Packaging",
                &format!("to `{}`", normalize_path(&pack_dir).display()),
            );
            if pack_dir.exists() {
                std::fs::remove_dir_all(&pack_dir)?;
            }
            std::fs::create_dir_all(&pack_dir)?;

            // CSS build
            crate::command::Command::new_npm_css_build(true).run()?;

            // Cargo build
            crate::command::Command::new_cargo_build(true).run()?;

            // Get the workspace root
            let workspace_root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("..");

            // Copy the binary to the target directory
            let binary_path = workspace_root
                .join("target")
                .join("release")
                .join("hagias-monitor-service.exe");
            let target_path = pack_dir.join("hagias-monitor-service.exe");
            crate::fs::copy_file(&binary_path, &target_path)?;

            // Copy layouts.json and Rocket.toml to the target directory
            for file in ["layouts.json", "Rocket.toml", "static/css/output.css"] {
                let src_path = workspace_root.join(file);
                let target_path = pack_dir.join(file);
                crate::fs::copy_file(&src_path, &target_path)?;
            }

            for dir in ["templates"] {
                let src_path = workspace_root.join(dir);
                let target_path = pack_dir.join(dir);
                crate::fs::copy_dir(&src_path, &target_path)?;
            }

            print_cargo_style("Finished", "packaging");
        }
        Commands::Run { release } => {
            // Build the CSS
            crate::command::Command::new_npm_css_build(release).run()?;

            // Build & run the monitor service
            crate::command::Command::new_cargo_run(release).run()?;
        }
        Commands::Watch { release } => {
            let workspace_root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("..");
            let (files, directories) =
                crate::ignore::get_unignored_files_and_directories(&workspace_root)?;

            let rt = tokio::runtime::Runtime::new().context("failed to create tokio runtime")?;
            rt.block_on(async {
                // TODO!
                // let error = Arc::new(Mutex::new(None::<anyhow::Error>));
                // let error_clone = error.clone();
                // let jobs_list =
                // let wx = Watchexec::new(move |mut action| {
                //     // Get the files that changed, If they are not ignored in the .gitignore, then rebuild everything
                //     let changed_directories = set_global_error_return(
                //         error_clone.clone(),
                //         get_changed_paths_from_action(
                //             action.events.clone(),
                //             watchexec_events::FileType::Dir,
                //         ),
                //     )
                //     .unwrap_or_else(|| HashSet::new());
                //     let changed_files = set_global_error_return(
                //         error_clone.clone(),
                //         get_changed_paths_from_action(
                //             action.events.clone(),
                //             watchexec_events::FileType::File,
                //         ),
                //     )
                //     .unwrap_or_else(|| HashSet::new());

                //     let have_any_unignored_paths_changed =
                //         have_any_unignored_paths_changed(&files, &changed_files)
                //             || have_any_unignored_paths_changed(&directories, &changed_directories);

                //     if have_any_unignored_paths_changed {
                //         // Kill all running builds
                //         // TODO

                //         // Create running builds again
                //         action.create_job(command)
                //     }

                //     // If Ctrl-C is received, quit
                //     if action.signals().any(|sig| sig == Signal::Interrupt) {
                //         action.quit();
                //     }
                //     action
                // })?;

                // // Set the filterer
                // wx.config.filterer(crate::watch::PathChangedFilterer);

                // // Watch the current directory
                // wx.config.pathset([workspace_root]);

                // // Run watchexec
                // wx.main()
                //     .await
                //     .context("failed to join watchexec")?
                //     .context("failed to run watchexec")?;

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
        }
    }
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
