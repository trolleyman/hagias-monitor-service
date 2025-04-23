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

            print_cargo_style(
                "Finished",
                &format!("packaging into `{}`", normalize_path(&pack_dir).display()),
            );
        }
        Commands::Run { release, args } => {
            // Build the CSS
            crate::command::Command::new_npm_css_build(release).run()?;

            // Build & run the monitor service
            crate::command::Command::new_cargo_run(release, args).run()?;
        }
        Commands::Watch { release, args } => crate::watch::run(release, args)?,
    }
    Ok(())
}
