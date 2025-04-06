use std::path::PathBuf;

use anyhow::{Context, Result};

use clap::Parser;
use display::DisplayConfig;
use serde::{Deserialize, Serialize};
use tokio::io::{AsyncReadExt, AsyncWriteExt};

pub mod display;
pub(crate) mod serde_override;

#[derive(Debug, Clone, clap::Parser)]
#[command(author, version, about)]
pub struct Args {
    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Debug, Clone, clap::Subcommand)]
pub enum Command {
    /// Edit configuration
    #[command(subcommand)]
    Config(ConfigCommand),
}

#[derive(Debug, Clone, clap::Subcommand)]
pub enum ConfigCommand {
    // Store the current monitor configuration as the config named `name`
    Store { id: String, name: String },
    // Apply the config with ID `id`
    Apply { id: String },
}

#[rocket::main]
pub async fn main() -> Result<()> {
    std::process::exit(run().await?)
}

pub async fn run() -> Result<i32> {
    let args = Args::parse();

    if let Some(command) = args.command {
        if let Some(code) = run_command(command).await? {
            return Ok(code);
        }
    }

    rocket::build().launch().await.context("Rocket error")?;
    Ok(0)
}

pub async fn run_command(command: Command) -> Result<Option<i32>> {
    match command {
        Command::Config(config_command) => match config_command {
            ConfigCommand::Store { id, name } => {
                store_monitor_config(&id, &name).await?;
                println!("Monitor config {} \"{}\" stored successfully", id, name);
                Ok(Some(0))
            }
            ConfigCommand::Apply { id } => {
                let stored_config = load_monitor_config(&id).await?;
                if let Some(stored_config) = stored_config {
                    println!(
                        "Monitor config {} \"{}\" loaded successfully",
                        stored_config.id, stored_config.name
                    );
                    stored_config.display_config.set()?;
                    println!(
                        "Monitor config {} \"{}\" applied successfully",
                        stored_config.id, stored_config.name
                    );
                    Ok(Some(0))
                } else {
                    println!("Monitor config {} not found", id);
                    Ok(Some(1))
                }
            }
        },
    }
}

fn get_display_config_path(id: &str) -> PathBuf {
    PathBuf::from("display_config").join(format!("{}.json", id))
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoredConfig {
    id: String,
    name: String,
    display_config: DisplayConfig,
}

pub async fn store_monitor_config(id: &str, name: &str) -> Result<()> {
    let windows_display_config =
        display::WindowsDisplayConfig::get(display::DisplayQueryType::All)?;

    let display_config = DisplayConfig::from_windows(&windows_display_config)?;
    let stored_config = StoredConfig {
        id: id.to_string(),
        name: name.to_string(),
        display_config,
    };

    let json = serde_json::to_string_pretty(&stored_config)?;

    let path = get_display_config_path(id);
    if let Some(parent) = path.parent() {
        tokio::fs::create_dir_all(parent).await?;
    }
    if tokio::fs::try_exists(&path).await? {
        println!("Overwriting monitor config {} \"{}\".", id, name);
    }
    let mut file = tokio::fs::File::create(path).await?;
    file.write_all(json.as_bytes()).await?;
    Ok(())
}

pub async fn load_monitor_config(id: &str) -> Result<Option<StoredConfig>> {
    let path = get_display_config_path(id);
    if !tokio::fs::try_exists(&path).await? {
        return Ok(None);
    }

    let mut file = tokio::fs::File::open(path).await?;
    let mut bytes = Vec::with_capacity(file.metadata().await?.len() as usize);
    file.read_to_end(&mut bytes).await?;
    let json = String::from_utf8(bytes)?;
    Ok(Some(serde_json::from_str(&json)?))
}
