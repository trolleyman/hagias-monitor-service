use anyhow::Result;
use clap::{Parser, Subcommand};
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
}

fn print_cargo_style(message: &str, action: &str) {
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
        &format!("`{}` to `{}`", src_normalized.display(), dest_dir.display()),
        "Copying",
    );
    std::fs::copy(src, dest)?;
    Ok(())
}

fn copy_dir(src: &Path, dest: &Path) -> Result<()> {
    let src_normalized = normalize_path(src);
    let dest_normalized = normalize_path(dest);
    let dest_dir = dest_normalized.parent().unwrap_or(&dest_normalized);

    print_cargo_style(
        &format!("`{}` to `{}`", src_normalized.display(), dest_dir.display()),
        "Copying",
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
            std::fs::copy(&src_path, &dest_path)?;
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
                &format!("to `{}`", normalize_path(&pack_dir).display()),
                "Packaging",
            );
            if pack_dir.exists() {
                std::fs::remove_dir_all(&pack_dir)?;
            }
            std::fs::create_dir_all(&pack_dir)?;

            // Build the release binary
            let cargo_path = env!("CARGO");
            print_cargo_style("hagias-monitor-service", "Building");
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
            copy_file(&binary_path, &target_path)?;

            // Copy layouts.json and Rocket.toml to the target directory
            for file in ["layouts.json", "Rocket.toml"] {
                let src_path = workspace_root.join(file);
                let target_path = pack_dir.join(file);
                copy_file(&src_path, &target_path)?;
            }

            for dir in ["templates"] {
                let src_path = workspace_root.join(dir);
                let target_path = pack_dir.join(dir);
                copy_dir(&src_path, &target_path)?;
            }

            print_cargo_style("package", "Finished");
        }
    }

    Ok(())
}
