use anyhow::{Context, Result};
use clap::Parser;
use config::Config;
use rocket_dyn_templates::Template;

pub mod command;
pub mod config;
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

    let figment = rocket::Config::figment();
    let config = figment
        .extract::<Config>()
        .context("Failed to extract config")?;

    if let Some(command) = args.command {
        if let Some(code) = command::run_command(command, &config).await? {
            return Ok(code);
        }
    }

    rocket::build()
        .configure(figment)
        .mount("/", rocket::routes![index::index, index::apply_config])
        .manage(config)
        .attach(Template::fairing())
        .launch()
        .await
        .context("Rocket error")?;
    Ok(0)
}
