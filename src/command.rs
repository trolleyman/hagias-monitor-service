use anyhow::Result;

use crate::config;

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
    // List all available configurations
    List,
}

pub async fn run_command(command: Command) -> Result<Option<i32>> {
    match command {
        Command::Config(config_command) => match config_command {
            ConfigCommand::Store { id, name } => {
                config::store_monitor_config(&id, &name).await?;
                println!("Monitor config {} \"{}\" stored successfully", id, name);
                Ok(Some(0))
            }
            ConfigCommand::Apply { id } => {
                let stored_config = config::load_monitor_config(&id).await?;
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
            ConfigCommand::List => {
                let configs = config::get_all_display_configs().await?;
                if configs.is_empty() {
                    println!("No monitor configurations found");
                } else {
                    println!("Available monitor configurations:");
                    for config in configs {
                        println!("  {} - {:?}", config.id, config.name);
                    }
                }
                Ok(Some(0))
            }
        },
    }
}
