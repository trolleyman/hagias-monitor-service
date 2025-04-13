use anyhow::Result;

use crate::config::Layouts;

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
                // TODO: Lock layouts
                println!("Loading layouts...");
                let mut layouts = Layouts::load().await?;
                layouts.add_current(&id, &name).await?;
                layouts.save().await?;
                println!("Monitor layout {} \"{}\" stored successfully", id, name);
                Ok(Some(0))
            }
            ConfigCommand::Apply { id } => {
                let layouts = Layouts::load().await?;
                let layout = layouts.get_layout(&id);
                if let Some(layout) = layout {
                    println!(
                        "Monitor layout {} \"{}\" loaded successfully",
                        layout.id, layout.name
                    );
                    layout.layout.apply()?;
                    println!(
                        "Monitor layout {} \"{}\" applied successfully",
                        layout.id, layout.name
                    );
                    Ok(Some(0))
                } else {
                    println!("Monitor layout {} not found", id);
                    Ok(Some(1))
                }
            }
            ConfigCommand::List => {
                let layouts = Layouts::load().await?;
                if layouts.is_empty() {
                    println!("No monitor configurations found");
                } else {
                    println!("Available monitor configurations:");
                    for layout in layouts {
                        println!("  {} - {:?}", layout.id, layout.name);
                    }
                }
                Ok(Some(0))
            }
        },
    }
}
