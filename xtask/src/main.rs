use anyhow::Result;
use clap::{Parser, Subcommand};
use std::path::PathBuf;
use std::process::Command;

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
        #[arg(short, long, default_value = "pack")]
        pack_dir: PathBuf,
    },
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Pack { pack_dir } => {
            // Create target directory if it doesn't exist
            eprintln!("Packaging to: {}", pack_dir.display());
            std::fs::create_dir_all(&pack_dir)?;

            // Build the release binary
            let cargo_path = env!("CARGO");
            eprintln!("Running cargo build --release");
            let status = Command::new(cargo_path)
                .args(["build", "--release"])
                .status()?;

            if !status.success() {
                anyhow::bail!("Failed to build release binary");
            }

            let workspace_root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("..");

            // Copy the binary to the target directory
            let binary_path = workspace_root
                .join("target")
                .join("release")
                .join("hagias-monitor-service.exe");
            let target_path = pack_dir.join("hagias-monitor-service.exe");

            eprintln!(
                "Copying {} to {}",
                binary_path.display(),
                target_path.display()
            );
            std::fs::copy(&binary_path, &target_path)?;

            // Copy layouts.json and Rocket.toml to the target directory
            let hagias_dir = workspace_root.join("hagias-monitor-service");
            for file in ["layouts.json", "Rocket.toml"] {
                let src_path = hagias_dir.join(file);
                let target_path = pack_dir.join(file);
                eprintln!(
                    "Copying {} to {}",
                    src_path.display(),
                    target_path.display()
                );
                std::fs::copy(&src_path, &target_path)?;
            }
        }
    }

    Ok(())
}
