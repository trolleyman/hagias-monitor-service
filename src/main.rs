use std::net::Ipv4Addr;

use anyhow::{Context, Result};
use clap::Parser;

pub mod command;
pub mod display;
pub mod index;
pub mod layouts;
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

    let config = rocket::Config {
        port: 5781,
        address: Ipv4Addr::new(0, 0, 0, 0).into(),
        ..rocket::Config::debug_default()
    };

    rocket::build()
        .configure(config)
        .mount("/", rocket::routes![index::index, index::apply_config])
        .launch()
        .await
        .context("Rocket error")?;
    Ok(0)
}
