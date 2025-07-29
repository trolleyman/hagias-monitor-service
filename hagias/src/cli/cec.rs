use anyhow::{Context, Result};
use cec_rs::CecConnectionCfgBuilder;

use crate::config::Config;

#[derive(Debug, Clone, clap::Subcommand)]
pub enum Command {
    /// Send a CEC command to a device
    #[command(subcommand)]
    Send(SendCommand),
}
impl Command {
    pub async fn run(&self, config: &Config) -> Result<Option<i32>> {
        match self {
            Command::Send(send_command) => send_command.run(config).await,
        }
    }
}

#[derive(Debug, Clone, clap::Subcommand)]
pub enum SendCommand {
    /// Power on a device
    PowerOn,
}

impl SendCommand {
    pub async fn run(&self, _config: &Config) -> Result<Option<i32>> {
        match self {
            SendCommand::PowerOn => {
                CecConnectionCfgBuilder::default()
                    .build()
                    .context("failed to connect to CEC device")?;
                Ok(Some(0))
            }
        }
    }
}
