use std::path::PathBuf;

use anyhow::Result;
use clap::Parser as _;

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
        /// Whether to pack the debug binary
        #[arg(short, long, default_value = "false")]
        debug: bool,
    },
    /// Run the monitor service
    Run {
        /// Whether to run the release binary
        #[arg(short, long, default_value = "false")]
        release: bool,
        /// Arguments to pass to the monitor service
        #[arg(last = true)]
        args: Vec<String>,
    },
    /// Run the monitor service and watch for changes
    Watch {
        /// Whether to run the release binary
        #[arg(short, long, default_value = "false")]
        release: bool,
        /// Arguments to pass to the monitor service
        #[arg(last = true)]
        args: Vec<String>,
    },
}

pub fn run() -> Result<i32> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Pack { pack_dir, debug } => {
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
            crate::command::Command::new_bun_css_build(!debug).run()?;

            // Cargo build
            crate::command::Command::new_cargo_build(!debug).run()?;

            // Get the workspace root
            let workspace_root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("..");

            // Copy the binary to the target directory
            for file_name in ["hagias.pdb", "hagias.exe"] {
                let binary_path = workspace_root
                    .join("target")
                    .join(if debug { "debug" } else { "release" })
                    .join(file_name);
                let target_path = pack_dir.join(file_name);
                crate::fs::copy_file(&binary_path, &target_path)?;
            }

            // Copy hagias/ files to the target directory
            for file in ["hagias_autohotkey.ahk"] {
                let src_path = workspace_root.join("hagias").join(file);
                let target_path = pack_dir.join(file);
                crate::fs::copy_file(&src_path, &target_path)?;
            }

            // Copy workspace files to the target directory
            for file in ["layouts.json", "Rocket.toml", "static/css/output.css"] {
                let src_path = workspace_root.join(file);
                let target_path = pack_dir.join(file);
                crate::fs::copy_file(&src_path, &target_path)?;
            }

            // Copy workspace directories to the target directory
            for dir in ["templates"] {
                let src_path = workspace_root.join(dir);
                let target_path = pack_dir.join(dir);
                crate::fs::copy_dir(&src_path, &target_path)?;
            }

            print_cargo_style(
                "Finished",
                &format!("packaging into `{}`", normalize_path(&pack_dir).display()),
            );
            Ok(0)
        }
        Commands::Run { release, args } => {
            // Build the CSS
            crate::command::Command::new_bun_css_build(release).run()?;

            // Build & run the monitor service
            let status = crate::command::Command::new_cargo_run(release, args).run_status()?;
            Ok(status.code().unwrap_or(1))
        }
        Commands::Watch { release, args } => crate::watch::run(release, args),
    }
}
