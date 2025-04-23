use anyhow::Result;
use crossterm::{
    QueueableCommand,
    cursor::{Hide, MoveToColumn, MoveToPreviousLine, Show},
    event::{Event, EventStream, KeyCode, KeyEvent, KeyEventKind, KeyModifiers},
    execute,
    style::Print,
    terminal::{Clear, ClearType, disable_raw_mode, enable_raw_mode},
};
use futures::StreamExt;
use std::{io::Write, path::PathBuf};
use tokio::io::AsyncBufReadExt;

use crate::{config::Config, layouts::Layouts};

#[derive(Debug, Clone, clap::Subcommand)]
pub enum Command {
    /// Edit layout configuration
    #[command(subcommand)]
    Layout(LayoutCommand),
    /// Run as a service
    #[command(subcommand)]
    Service,
}

#[derive(Debug, Clone, clap::Subcommand)]
pub enum LayoutCommand {
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

pub async fn run_command(command: Command, config: &Config) -> Result<Option<i32>> {
    match command {
        Command::Layout(layout_command) => match layout_command {
            LayoutCommand::Store { id, name, emoji } => {
                // TODO: Lock layouts
                println!("Loading layouts...");
                let mut layouts = Layouts::load(&config.layouts_path.relative()).await?;
                layouts.add_current(&id, &name, emoji.as_deref()).await?;
                layouts.save(&config.layouts_path.relative()).await?;
                println!("Monitor layout {} \"{}\" stored successfully", id, name);
                Ok(Some(0))
            }
            LayoutCommand::Apply { id } => {
                let layouts = Layouts::load(&config.layouts_path.relative()).await?;
                let layout = layouts.get_layout(&id);
                if let Some(layout) = layout {
                    println!(
                        "Monitor layout {} \"{}\" loaded successfully",
                        layout.id, layout.name
                    );
                    layout.layout.apply(true)?;
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
            LayoutCommand::List => {
                let layouts = Layouts::load(&config.layouts_path.relative()).await?;
                if layouts.is_empty() {
                    println!("No monitor configurations found");
                } else {
                    println!("Available monitor configurations:");
                    for layout in layouts {
                        println!(
                            "  {} - {:?}{}{}",
                            layout.id,
                            layout.name,
                            layout
                                .emoji
                                .map(|s| format!(" ({})", s))
                                .unwrap_or_default(),
                            if layout.hidden { " [hidden]" } else { "" }
                        );
                    }
                }
                Ok(Some(0))
            }
            LayoutCommand::Rearrange => {
                let mut layouts = Layouts::load(&config.layouts_path.relative()).await?;
                if layouts.is_empty() {
                    println!("No monitor configurations found to rearrange");
                    return Ok(Some(1));
                }
                let mut stdout = std::io::stdout();
                let mut rearranger =
                    Rearranger::new(&mut layouts, config.layouts_path.relative(), &mut stdout);
                rearranger.run().await?;
                Ok(Some(0))
            }
            LayoutCommand::Hide { id } => {
                let mut layouts = Layouts::load(&config.layouts_path.relative()).await?;
                if let Some(layout) = layouts.get_layout_mut(&id) {
                    let id = layout.id.clone();
                    let name = layout.name.clone();
                    layout.hidden = true;
                    layouts.save(&config.layouts_path.relative()).await?;
                    println!("Monitor layout {} \"{}\" hidden successfully", id, name);
                    Ok(Some(0))
                } else {
                    println!("Monitor layout {} not found", id);
                    Ok(Some(1))
                }
            }
            LayoutCommand::Unhide { id } => {
                let mut layouts = Layouts::load(&config.layouts_path.relative()).await?;
                if let Some(layout) = layouts.get_layout_mut(&id) {
                    let id = layout.id.clone();
                    let name = layout.name.clone();
                    layout.hidden = false;
                    layouts.save(&config.layouts_path.relative()).await?;
                    println!("Monitor layout {} \"{}\" unhidden successfully", id, name);
                    Ok(Some(0))
                } else {
                    println!("Monitor layout {} not found", id);
                    Ok(Some(1))
                }
            }
        },
        Command::Service => {
            todo!()
        }
    }
}

struct Rearranger<'a> {
    layouts: &'a mut Layouts,
    layouts_path: PathBuf,
    stdout: &'a mut std::io::Stdout,
    selected: usize,
    grabbed: bool,
    has_changes: bool,
    current_line: usize,
    status: Option<String>,
}

impl<'a> Rearranger<'a> {
    fn new(
        layouts: &'a mut Layouts,
        layouts_path: PathBuf,
        stdout: &'a mut std::io::Stdout,
    ) -> Self {
        Self {
            layouts,
            layouts_path,
            stdout,
            selected: 0,
            grabbed: false,
            has_changes: false,
            current_line: 0,
            status: None,
        }
    }

    fn move_to_line(&mut self, line: usize) -> Result<()> {
        if line > self.layouts.len() {
            return Err(anyhow::anyhow!("Invalid line: {}", line));
        } else if line == self.current_line {
            return Ok(());
        } else if line < self.current_line {
            execute!(
                self.stdout,
                MoveToPreviousLine((self.current_line - line) as u16)
            )?;
        } else {
            for _ in 0..(line - self.current_line) {
                self.stdout.queue(Print("\n"))?;
            }
            self.stdout.flush()?;
        }
        self.current_line = line;
        Ok(())
    }

    pub async fn run(&mut self) -> Result<()> {
        writeln!(self.stdout, "Controls:")?;
        writeln!(self.stdout, "  ↑/↓ - Move selection up/down")?;
        writeln!(self.stdout, "  Space - Grab/ungrab selected layout")?;
        writeln!(self.stdout, "  s - Save changes")?;
        writeln!(self.stdout, "  q - Quit")?;
        writeln!(self.stdout)?;

        enable_raw_mode()?;
        execute!(self.stdout, Hide)?;

        self.current_line = 0;
        self.update_all_lines()?;

        let mut reader = EventStream::new();
        loop {
            if let Some(Ok(Event::Key(KeyEvent {
                code,
                modifiers,
                kind: KeyEventKind::Press,
                ..
            }))) = reader.next().await
            {
                self.set_status(None)?;
                match code {
                    KeyCode::Char('q') => break,
                    KeyCode::Char('s') => {
                        self.set_status(Some("Saving changes...".into()))?;

                        self.layouts.save(&self.layouts_path).await?;
                        self.has_changes = false;

                        self.set_status(Some("Changes saved successfully".into()))?;
                    }
                    KeyCode::Char(' ') => {
                        self.grabbed = !self.grabbed;
                        self.update_line(self.selected)?;
                    }
                    KeyCode::Up => {
                        if self.selected > 0 {
                            if self.grabbed {
                                self.layouts.swap(self.selected, self.selected - 1);
                                self.has_changes = true;
                            }
                            self.selected -= 1;

                            // Update both the previously selected and newly selected items
                            self.update_line(self.selected + 1)?;
                            self.update_line(self.selected)?;
                        }
                    }
                    KeyCode::Down => {
                        if self.selected < self.layouts.len() - 1 {
                            if self.grabbed {
                                self.layouts.swap(self.selected, self.selected + 1);
                                self.has_changes = true;
                            }
                            self.selected += 1;

                            // Update both the previously selected and newly selected items
                            self.update_line(self.selected - 1)?;
                            self.update_line(self.selected)?;
                        }
                    }
                    KeyCode::Char('c') if modifiers.contains(KeyModifiers::CONTROL) => {
                        // Ctrl+C (interrupt)
                        break;
                    }
                    _ => {}
                }
            }
        }

        self.status = None;
        self.move_to_line(self.layouts.len())?;
        self.update_current_line()?;
        execute!(self.stdout, Print("\n"), Show)?;
        disable_raw_mode()?;

        if self.has_changes {
            println!("Save changes? (y/n)");
            let mut input = String::new();
            tokio::io::BufReader::new(tokio::io::stdin())
                .read_line(&mut input)
                .await?;
            if input.trim().to_lowercase() == "y" {
                self.layouts.save(&self.layouts_path).await?;
                println!("Changes saved successfully");
            } else {
                println!("Changes discarded");
            }
        }

        Ok(())
    }

    fn set_status(&mut self, new_status: Option<String>) -> Result<()> {
        if self.status == new_status {
            return Ok(());
        }

        self.status = new_status;
        self.move_to_line(self.layouts.len())?;
        self.update_current_line()?;
        Ok(())
    }

    fn update_line(&mut self, line: usize) -> Result<()> {
        self.move_to_line(line)?;
        self.update_current_line()?;
        Ok(())
    }

    fn update_current_line(&mut self) -> Result<()> {
        if self.current_line == self.layouts.len() {
            if let Some(status) = &self.status {
                execute!(
                    self.stdout,
                    MoveToColumn(0),
                    Clear(ClearType::CurrentLine),
                    Print(status)
                )?;
            } else {
                execute!(self.stdout, MoveToColumn(0), Clear(ClearType::CurrentLine))?;
            }
        } else {
            let prefix = if self.selected == self.current_line {
                if self.grabbed { " [X] " } else { " [ ] " }
            } else {
                "     "
            };
            execute!(
                self.stdout,
                MoveToColumn(0),
                Clear(ClearType::CurrentLine),
                Print(format!(
                    "{}{}. {} ({})",
                    prefix,
                    self.current_line + 1,
                    self.layouts[self.current_line].name,
                    self.layouts[self.current_line].id
                ))
            )?;
        }
        Ok(())
    }

    fn update_all_lines(&mut self) -> Result<()> {
        let current_line = self.current_line;
        for i in 0..self.layouts.len() {
            self.move_to_line(i)?;
            self.update_current_line()?;
        }
        self.move_to_line(current_line)?;
        Ok(())
    }
}
