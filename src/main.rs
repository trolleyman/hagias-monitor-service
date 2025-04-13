use anyhow::{Context, Result};
use clap::Parser;

pub mod command;
pub mod config;
pub mod display;
pub mod index;
pub(crate) mod serde_override;
pub mod windows_util;

#[derive(Debug, Clone, clap::Parser)]
#[command(author, version, about)]
pub struct Args {
    #[command(subcommand)]
    command: Option<command::Command>,
}

#[rocket::main]
pub async fn main() -> Result<()> {
    std::process::exit(run().await?)
}

pub async fn run() -> Result<i32> {
    let args = Args::parse();

    if let Some(command) = args.command {
        if let Some(code) = command::run_command(command).await? {
            return Ok(code);
        }
    }

    rocket::build()
        .mount("/", rocket::routes![index::index, index::apply_config])
        .launch()
        .await
        .context("Rocket error")?;
    Ok(0)
}
