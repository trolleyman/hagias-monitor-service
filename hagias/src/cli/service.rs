use anyhow::Result;
use tracing::info;

use crate::config::Config;

#[derive(Debug, Clone, clap::Subcommand)]
pub enum Command {
    /// Register the service, starting it immediately
    Register {
        /// Force the registration of the new service (overwrite existing service)
        #[arg(short, long)]
        force: bool,
        /// Don't start the service immediately
        #[arg(short, long)]
        no_start: bool,
    },
    /// Unregister the service
    Unregister,
    /// Run the service
    ///
    /// This should only be called by Windows
    Run,
    /// Start the service
    Start,
    /// Stop the service
    Stop,
    /// Restart the service
    Restart,
    /// Get the status of the service
    Status,
}

impl Command {
    pub async fn run(&self, config: &Config) -> Result<Option<i32>> {
        match self {
            Command::Register { force, no_start } => {
                if *force {
                    info!("Unregistering service if it exists...");
                    crate::service::unregister_if_exists().await?;
                }
                info!("Registering service...");
                crate::service::register(!no_start).await?;
                info!("Service registered successfully");
                if !no_start {
                    info!(
                        "Hagias should be now available at http://localhost:{}",
                        config.port
                    );
                }
                Ok(Some(0))
            }
            Command::Unregister => {
                info!("Unregistering service...");
                crate::service::unregister().await?;
                info!("Service unregistered successfully");
                Ok(Some(0))
            }
            Command::Run => {
                info!("Running service...");
                crate::service::run()?;
                Ok(Some(0))
            }
            Command::Start => {
                info!("Starting service...");
                crate::service::start().await?;
                info!("Service started successfully");
                info!(
                    "Hagias should be now available at http://localhost:{}",
                    config.port
                );
                Ok(Some(0))
            }
            Command::Stop => {
                info!("Stopping service...");
                crate::service::stop().await?;
                info!("Service stopped successfully");
                Ok(Some(0))
            }
            Command::Restart => {
                info!("Restarting service...");
                crate::service::restart().await?;
                info!("Service restarted successfully");
                info!(
                    "Hagias should be now available at http://localhost:{}",
                    config.port
                );
                Ok(Some(0))
            }
            Command::Status => {
                match crate::service::status().await? {
                    Some(status) => info!("Service status: {:?}", status.current_state),
                    None => info!("Service is not running"),
                }
                Ok(Some(0))
            }
        }
    }
}
