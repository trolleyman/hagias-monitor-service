use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use std::ffi::OsStr;
use std::fmt::Display;
use std::path::{Path, PathBuf};
use std::process::Command;

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

fn print_cargo_style(action: impl Display, message: impl Display) {
    println!("{}{:>12} {}{}", GREEN_BOLD, action, RESET, message);
}

fn canonicalize_or_original(path: &Path) -> PathBuf {
    std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf())
}

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
            .with_context(|| format!("Failed to create directory `{}`", dest_parent.display()))?;
    }
    std::fs::copy(src, dest).with_context(|| {
        format!(
            "Failed to copy `{}` to `{}`",
            src_normalized.display(),
            dest_normalized.display()
        )
    })?;
    Ok(())
}

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
                    "Failed to copy `{}` to `{}`",
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
            todo!()
        }
    }

    Ok(())
}

fn run_command<S1, S2, I>(command: S1, args: I) -> Result<(), anyhow::Error>
where
    S1: AsRef<OsStr>,
    S2: AsRef<OsStr>,
    I: IntoIterator<Item = S2>,
{
    let command = command.as_ref();
    let command_normalized = normalize_path(&Path::new(command));
    let args = args
        .into_iter()
        .map(|s| s.as_ref().to_owned())
        .collect::<Vec<_>>();
    let command_full = format!(
        "{} {}",
        command_normalized.display(),
        args.iter()
            .map(|arg| arg.display().to_string())
            .map(|arg| if arg.contains(" ") {
                format!("\"{}\"", arg)
            } else {
                arg
            })
            .collect::<Vec<_>>()
            .join(" ")
    );
    print_cargo_style("Running", &command_full);
    let status = Command::new(&command)
        .args(&args)
        .status()
        .with_context(|| format!("Failed to start command: {}", &command_full))?;
    Ok(if !status.success() {
        anyhow::bail!(
            "Command returned exit code {}: {}",
            status.code().unwrap_or(-1),
            &command_full
        );
    })
}
