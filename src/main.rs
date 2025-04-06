use anyhow::{Context, Result};

use clap::Parser;

pub mod display;

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
                Ok(Some(0))
            }
        },
    }
}

pub async fn store_monitor_config(_id: &str, _name: &str) -> Result<()> {
    let windows_display_config =
        display::WindowsDisplayConfig::get(display::DisplayQueryType::Active)?;
    windows_display_config.print();

    // windows_display_config.set()?;

    Ok(())
}
