use crate::layouts::Layouts;
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

pub struct Rearranger<'a> {
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
    pub(crate) fn new(
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

    pub(crate) fn move_to_line(&mut self, line: usize) -> Result<()> {
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

    pub(crate) fn set_status(&mut self, new_status: Option<String>) -> Result<()> {
        if self.status == new_status {
            return Ok(());
        }

        self.status = new_status;
        self.move_to_line(self.layouts.len())?;
        self.update_current_line()?;
        Ok(())
    }

    pub(crate) fn update_line(&mut self, line: usize) -> Result<()> {
        self.move_to_line(line)?;
        self.update_current_line()?;
        Ok(())
    }

    pub(crate) fn update_current_line(&mut self) -> Result<()> {
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

    pub(crate) fn update_all_lines(&mut self) -> Result<()> {
        let current_line = self.current_line;
        for i in 0..self.layouts.len() {
            self.move_to_line(i)?;
            self.update_current_line()?;
        }
        self.move_to_line(current_line)?;
        Ok(())
    }
}
