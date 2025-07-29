use anyhow::Result;
use tracing::{error, info};

use crate::{config::Config, layouts::Layouts};

use super::rearranger::Rearranger;

#[derive(Debug, Clone, clap::Subcommand)]
pub enum Command {
    // Store the current monitor configuration as the config named `name`
    Store {
        /// The ID of the layout
        id: String,
        /// The human-readable name of the layout
        name: String,
        /// The emoji to display for the layout
        #[arg(short, long)]
        emoji: Option<String>,
    },
    // Apply the config with ID `id`
    Apply {
        /// The ID of the layout
        id: String,
    },
    // List all available configurations
    List,
    // Interactively rearrange monitor layouts
    Rearrange,
    // Hide a layout
    Hide {
        /// The ID of the layout to hide
        id: String,
    },
    // Unhide a layout
    Unhide {
        /// The ID of the layout to unhide
        id: String,
    },
}

impl Command {
    pub async fn run(&self, config: &Config) -> Result<Option<i32>> {
        match self {
            Command::Store { id, name, emoji } => {
                // TODO: Lock layouts
                info!("Loading layouts...");
                let mut layouts = Layouts::load(&config.layouts_path.relative()).await?;
                layouts.add_current(&id, &name, emoji.as_deref()).await?;
                layouts.save(&config.layouts_path.relative()).await?;
                info!("Monitor layout {} \"{}\" stored successfully", id, name);
                Ok(Some(0))
            }
            Command::Apply { id } => {
                let layouts = Layouts::load(&config.layouts_path.relative()).await?;
                let layout = id
                    .parse::<usize>()
                    .ok()
                    .map(|index| {
                        if index == 0 {
                            None
                        } else {
                            layouts.get_layout_by_index(index - 1)
                        }
                    })
                    .unwrap_or_else(|| layouts.get_layout(&id));
                if let Some(layout) = layout {
                    info!(
                        "Monitor layout {} \"{}\" loaded successfully",
                        layout.id, layout.name
                    );
                    layout.layout.apply(true)?;
                    info!(
                        "Monitor layout {} \"{}\" applied successfully",
                        layout.id, layout.name
                    );
                    Ok(Some(0))
                } else {
                    error!("Monitor layout {} not found", id);
                    Ok(Some(1))
                }
            }
            Command::List => {
                let layouts = Layouts::load(&config.layouts_path.relative()).await?;
                if layouts.is_empty() {
                    info!("No monitor configurations found");
                } else {
                    info!("Available monitor configurations:");
                    for (i, layout) in layouts.iter().enumerate() {
                        info!(
                            "  {}. {} - {:?}{}{}",
                            i + 1,
                            layout.id,
                            layout.name,
                            if layout.hidden { " [hidden]" } else { "" },
                            layout
                                .emoji
                                .as_ref()
                                .map(|s| format!(" {}", s))
                                .unwrap_or_default(),
                        );
                    }
                }
                Ok(Some(0))
            }
            Command::Rearrange => {
                let mut layouts = Layouts::load(&config.layouts_path.relative()).await?;
                if layouts.is_empty() {
                    error!("No monitor configurations found to rearrange");
                    return Ok(Some(1));
                }
                let mut stdout = std::io::stdout();
                let mut rearranger =
                    Rearranger::new(&mut layouts, config.layouts_path.relative(), &mut stdout);
                rearranger.run().await?;
                Ok(Some(0))
            }
            Command::Hide { id } => {
                let mut layouts = Layouts::load(&config.layouts_path.relative()).await?;
                if let Some(layout) = layouts.get_layout_mut(&id) {
                    let id = layout.id.clone();
                    let name = layout.name.clone();
                    layout.hidden = true;
                    layouts.save(&config.layouts_path.relative()).await?;
                    info!("Monitor layout {} \"{}\" hidden successfully", id, name);
                    Ok(Some(0))
                } else {
                    error!("Monitor layout {} not found", id);
                    Ok(Some(1))
                }
            }
            Command::Unhide { id } => {
                let mut layouts = Layouts::load(&config.layouts_path.relative()).await?;
                if let Some(layout) = layouts.get_layout_mut(&id) {
                    let id = layout.id.clone();
                    let name = layout.name.clone();
                    layout.hidden = false;
                    layouts.save(&config.layouts_path.relative()).await?;
                    info!("Monitor layout {} \"{}\" unhidden successfully", id, name);
                    Ok(Some(0))
                } else {
                    error!("Monitor layout {} not found", id);
                    Ok(Some(1))
                }
            }
        }
    }
}
