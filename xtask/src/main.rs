use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use command_group::{CommandGroup, GroupChild};
use notify_debouncer_full::{DebounceEventResult, notify};
use std::collections::HashSet;
use std::ffi::{OsStr, OsString};
use std::fmt::Display;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;

const GREEN_BOLD: &str = "\x1b[1;32m";
const RESET: &str = "\x1b[0m";

#[derive(Parser)]
#[command(author, version, about, long_about = None)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
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

/// Print a message in cargo style
fn print_cargo_style(action: impl Display, message: impl Display) {
    println!("{}{:>12} {}{}", GREEN_BOLD, action, RESET, message);
}

/// Canonicalize a path, or return the original path if it fails
fn canonicalize_or_original(path: &Path) -> PathBuf {
    std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf())
}

/// Normalize a path to be relative to the current directory, to make it easier to read
fn normalize_path(path: &Path) -> PathBuf {
    let current_dir = match std::env::current_dir() {
        Ok(dir) => canonicalize_or_original(&dir),
        Err(_) => return path.to_path_buf(),
    };

    let canonical_path = canonicalize_or_original(path);

    if let Ok(relative) = canonical_path.strip_prefix(&current_dir) {
        return relative.to_path_buf();
    }
    if let Ok(relative) = path.strip_prefix(&current_dir) {
        return relative.to_path_buf();
    }
    canonical_path
}

/// Copy a file, printing a message in cargo style
fn copy_file(src: &Path, dest: &Path) -> Result<()> {
    let src_normalized = normalize_path(src);
    let dest_normalized = normalize_path(dest);
    let dest_dir = dest_normalized.parent().unwrap_or(&dest_normalized);

    print_cargo_style(
        "Copying",
        &format!("`{}` to `{}`", src_normalized.display(), dest_dir.display()),
    );
    if let Some(dest_parent) = dest_normalized.parent() {
        std::fs::create_dir_all(dest_parent)
            .with_context(|| format!("failed to create directory `{}`", dest_parent.display()))?;
    }
    std::fs::copy(src, dest).with_context(|| {
        format!(
            "failed to copy `{}` to `{}`",
            src_normalized.display(),
            dest_normalized.display()
        )
    })?;
    Ok(())
}

/// Copy a directory, printing a message in cargo style
fn copy_dir(src: &Path, dest: &Path) -> Result<()> {
    let src_normalized = normalize_path(src);
    let dest_normalized = normalize_path(dest);
    let dest_dir = dest_normalized.parent().unwrap_or(&dest_normalized);

    print_cargo_style(
        "Copying",
        &format!("`{}` to `{}`", src_normalized.display(), dest_dir.display()),
    );

    copy_dir_silent(src, dest)?;
    Ok(())
}

/// Copy a directory silently, without printing a message in cargo style
fn copy_dir_silent(src: &Path, dest: &Path) -> Result<()> {
    std::fs::create_dir_all(dest)?;
    for entry in std::fs::read_dir(src)? {
        let entry = entry?;
        let src_path = entry.path();
        let dest_path = dest.join(src_path.file_name().unwrap());
        if src_path.is_dir() {
            copy_dir_silent(&src_path, &dest_path)?;
        } else {
            std::fs::copy(&src_path, &dest_path).with_context(|| {
                format!(
                    "failed to copy `{}` to `{}`",
                    src_path.display(),
                    dest_path.display()
                )
            })?;
        }
    }
    Ok(())
}

fn main() -> Result<()> {
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

            // Build the release binary
            run_command(env!("CARGO"), ["build", "--release"])?;

            // Build the CSS
            run_command("npm.cmd", ["run", "build-release:css"])?;

            // Get the workspace root
            let workspace_root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("..");

            // Copy the binary to the target directory
            let binary_path = workspace_root
                .join("target")
                .join("release")
                .join("hagias-monitor-service.exe");
            let target_path = pack_dir.join("hagias-monitor-service.exe");
            copy_file(&binary_path, &target_path)?;

            // Copy layouts.json and Rocket.toml to the target directory
            for file in ["layouts.json", "Rocket.toml", "static/css/output.css"] {
                let src_path = workspace_root.join(file);
                let target_path = pack_dir.join(file);
                copy_file(&src_path, &target_path)?;
            }

            for dir in ["templates"] {
                let src_path = workspace_root.join(dir);
                let target_path = pack_dir.join(dir);
                copy_dir(&src_path, &target_path)?;
            }

            print_cargo_style("Finished", "packaging");
        }
        Commands::Run { release } => {
            // Build the CSS
            let npm_build_css_script_name = if release {
                "build-release:css"
            } else {
                "build:css"
            };
            run_command("npm.cmd", ["run", npm_build_css_script_name])?;

            // Run the monitor service
            let cargo_path = env!("CARGO");
            let args = if release {
                vec!["run", "--release"]
            } else {
                vec!["run"]
            };
            run_command(cargo_path, args)?;
        }
        Commands::Watch { release } => {
            enum WatchEvent {
                ChangedPaths(HashSet<PathBuf>),
                Error(anyhow::Error),
            }
            let (tx, rx) = std::sync::mpsc::channel();
            let mut debouncer = notify_debouncer_full::new_debouncer(
                Duration::from_millis(200),
                None,
                move |res: DebounceEventResult| match res {
                    Ok(events) => {
                        let changed_paths = events
                            .iter()
                            .filter(|e| {
                                e.event.kind.is_create()
                                    || e.event.kind.is_modify()
                                    || e.event.kind.is_remove()
                            })
                            .flat_map(|e| e.paths.clone())
                            .collect();
                        tx.send(WatchEvent::ChangedPaths(changed_paths)).unwrap();
                    }
                    Err(mut e) => {
                        let error = match e.len() {
                            0 => anyhow::anyhow!("watch error"),
                            1 => anyhow::anyhow!(e.remove(0)).context("watch error"),
                            _ => anyhow::anyhow!(e.remove(0))
                                .context(format!("other watch errors: {:?}", e)),
                        };
                        tx.send(WatchEvent::Error(error)).unwrap();
                    }
                },
            )
            .context("failed to create notify debouncer")?;

            let workspace_root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("..");
            debouncer
                .watch(&workspace_root, notify::RecursiveMode::Recursive)
                .with_context(|| format!("failed to watch {}", workspace_root.display()))?;

            loop {
                // Build CSS
                let npm_build_css_script_name = if release {
                    "build-release:css"
                } else {
                    "build:css"
                };
                run_command("npm.cmd", ["run", npm_build_css_script_name])?;

                // Start service
                let cargo_path = env!("CARGO");
                let args = if release {
                    vec!["build", "--release"]
                } else {
                    vec!["build"]
                };
                run_command(cargo_path, args)?;

                // Start service
                let mut service_path = workspace_root.join("target");
                if release {
                    service_path = service_path.join("release");
                } else {
                    service_path = service_path.join("debug");
                }
                service_path = service_path.join("hagias-monitor-service.exe");
                let mut child =
                    run_command_background::<&Path, &str, [&str; 0]>(&service_path, [])?;

                // Wait for changes
                let result = loop {
                    match rx.recv() {
                        Ok(WatchEvent::ChangedPaths(paths)) => {
                            // If path is in paths that trigger a rebuild, rebuild
                            if paths
                                .iter()
                                .any(|path| path.starts_with(&workspace_root.join("src")))
                            {
                                break Ok(());
                            }
                        }
                        Ok(WatchEvent::Error(error)) => {
                            break Err(anyhow::anyhow!(error).context("watch error"));
                        }
                        Err(e) => {
                            break Err(anyhow::anyhow!(e).context("recv error"));
                        }
                    }
                };

                // Kill process
                child.kill().context("failed to kill service")?;

                if let Err(e) = result {
                    return Err(e);
                }
            }
        }
    }
    Ok(())
}

/// Check if a path is in another path, accounting for `..` and other elements
fn is_path_in(container: &Path, path: &Path) -> bool {
    let container_normalized = canonicalize_or_original(container);
    let path_normalized = canonicalize_or_original(path);
    path_normalized.starts_with(&container_normalized)
}

/// Run a command as a group child
fn run_command_debug<S1, S2, I>(command: S1, args: I) -> Result<(GroupChild, String), anyhow::Error>
where
    S1: AsRef<OsStr>,
    S2: AsRef<OsStr>,
    I: IntoIterator<Item = S2>,
{
    let command = command.as_ref().to_owned();
    let command_normalized = normalize_path(&Path::new(&command));
    let args = args
        .into_iter()
        .map(|s| s.as_ref().to_owned())
        .collect::<Vec<_>>();
    let command_full_debug = format!(
        "{} {}",
        command_normalized.display(),
        args.iter()
            .map(|arg| arg.display().to_string())
            .collect::<Vec<_>>()
            .join(" ")
    );
    print_cargo_style("Running", &command_full_debug);
    let group_child = Command::new(&command)
        .args(&args)
        .group_spawn()
        .with_context(|| format!("failed to start command: {}", &command_full_debug))?;
    Ok((group_child, command_full_debug))
}

/// Run a command and wait for it to exit, throwing an error if it returns a non-zero exit code
fn run_command<S1, S2, I>(command: S1, args: I) -> Result<(), anyhow::Error>
where
    S1: AsRef<OsStr>,
    S2: AsRef<OsStr>,
    I: IntoIterator<Item = S2>,
{
    let (mut group_child, command_full_debug) = run_command_debug(command, args)?;
    let status = group_child
        .wait()
        .with_context(|| format!("failed to wait for command: {}", &command_full_debug))?;
    Ok(if !status.success() {
        anyhow::bail!(
            "command returned exit code {}: {}",
            status.code().unwrap_or(-1),
            &command_full_debug
        );
    })
}

/// Run a command in the background
fn run_command_background<S1, S2, I>(command: S1, args: I) -> Result<GroupChild, anyhow::Error>
where
    S1: AsRef<OsStr>,
    S2: AsRef<OsStr>,
    I: IntoIterator<Item = S2>,
{
    Ok(run_command_debug(command, args)?.0)
}
