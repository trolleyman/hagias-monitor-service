use anyhow::Result;
use crossterm::{
    cursor::{Hide, Show},
    event::{self, Event, KeyCode, KeyEvent},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, Clear, ClearType},
};

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
    // Interactively rearrange monitor layouts
    Rearrange,
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
            ConfigCommand::Rearrange => {
                let mut layouts = Layouts::load().await?;
                if layouts.is_empty() {
                    println!("No monitor configurations found to rearrange");
                    return Ok(Some(1));
                }

                // Enable raw mode and hide cursor
                enable_raw_mode()?;
                let mut stdout = std::io::stdout();
                execute!(stdout, Hide)?;

                let mut selected = 0;
                let mut grabbed = None;
                let mut has_changes = false;

                loop {
                    // Clear screen and redraw
                    execute!(stdout, Clear(ClearType::All))?;
                    println!("Controls:");
                    println!("  ↑/↓ - Move selection up/down");
                    println!("  Space - Grab/ungrab selected layout");
                    println!("  s - Save changes");
                    println!("  q - Quit");
                    println!();
                    println!("Current order:");
                    for (i, layout) in layouts.iter().enumerate() {
                        let prefix = if Some(i) == grabbed {
                            "> "
                        } else if i == selected {
                            "* "
                        } else {
                            "  "
                        };
                        println!("{}{}. {} - {}", prefix, i + 1, layout.id, layout.name);
                    }

                    // Read key
                    if let Event::Key(KeyEvent { code, .. }) = event::read()? {
                        match code {
                            KeyCode::Char('q') => break,
                            KeyCode::Char('s') => {
                                layouts.save().await?;
                                has_changes = false;
                                println!("Changes saved successfully");
                                std::thread::sleep(std::time::Duration::from_millis(1000));
                            }
                            KeyCode::Char(' ') => {
                                if grabbed.is_none() {
                                    grabbed = Some(selected);
                                } else {
                                    grabbed = None;
                                }
                            }
                            KeyCode::Up => {
                                if let Some(g) = grabbed {
                                    if g > 0 {
                                        layouts.swap(g, g - 1);
                                        grabbed = Some(g - 1);
                                        has_changes = true;
                                    }
                                } else if selected > 0 {
                                    selected -= 1;
                                }
                            }
                            KeyCode::Down => {
                                if let Some(g) = grabbed {
                                    if g < layouts.len() - 1 {
                                        layouts.swap(g, g + 1);
                                        grabbed = Some(g + 1);
                                        has_changes = true;
                                    }
                                } else if selected < layouts.len() - 1 {
                                    selected += 1;
                                }
                            }
                            _ => {}
                        }
                    }
                }

                // Restore terminal state
                execute!(stdout, Show)?;
                disable_raw_mode()?;

                if has_changes {
                    println!("Save changes? (y/n)");
                    let mut input = String::new();
                    std::io::stdin().read_line(&mut input)?;
                    if input.trim().to_lowercase() == "y" {
                        layouts.save().await?;
                        println!("Changes saved successfully");
                    } else {
                        println!("Changes discarded");
                    }
                }

                Ok(Some(0))
            }
        },
    }
}
