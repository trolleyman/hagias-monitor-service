use anyhow::{Context, Result};

use clap::Parser;
use display::DisplayConfig;
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
}

#[rocket::main]
pub async fn main() {
    match run().await {
        Ok(code) => std::process::exit(code),
        Err(e) => {
            eprintln!("Error: {}", e);
            std::process::exit(1);
        }
    }
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
                println!("Monitor config stored successfully");
                Ok(Some(0))
            }
        },
    }
}

pub async fn store_monitor_config(_id: &str, _name: &str) -> Result<()> {
    println!("Getting current display configuration...");
    let windows_display_config =
        display::WindowsDisplayConfig::get(display::DisplayQueryType::All)?;
    println!("Current display configuration:");
    windows_display_config.print();

    // windows_display_config.set()?;

    println!("Converting to DisplayConfig...");
    let display_config = DisplayConfig::from_windows(&windows_display_config)?;

    println!("Serializing to JSON...");
    let json = serde_json::to_string_pretty(&display_config)?;

    println!("Writing to display_config.json...");
    let mut file = tokio::fs::File::create("display_config.json").await?;
    file.write_all(json.as_bytes()).await?;

    println!("Reading back display_config.json...");
    let mut bytes = Vec::with_capacity(json.len());
    println!("Reading to end...");
    let mut file = tokio::fs::File::open("display_config.json").await?;
    file.read_to_end(&mut bytes).await?;
    println!("Converting to string...");
    let json = String::from_utf8(bytes)?;

    println!("Deserializing from JSON...");
    // println!("JSON: {}", json);
    let display_config: DisplayConfig = match serde_json::from_str(&json) {
        Ok(display_config) => {
            println!("DisplayConfig: {:#?}", display_config);
            display_config
        }
        Err(e) => {
            eprintln!("Error: {}", e);
            anyhow::bail!(e);
        }
    };

    println!("Setting display configuration...");
    display_config.set()?;
    println!("Display configuration set successfully");

    Ok(())
}
