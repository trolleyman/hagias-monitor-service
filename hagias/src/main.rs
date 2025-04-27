use std::sync::LazyLock;

use anyhow::{Context, Result};
use clap::Parser;
use rocket::fs::FileServer;
use rocket_dyn_templates::Template;
use tracing::debug;

pub mod command;
pub mod config;
pub mod display;
pub mod index;
pub mod layouts;
pub mod logging;
pub mod serde_override;
pub mod service;
pub mod windows_util;

static TOKIO_RUNTIME: LazyLock<Result<tokio::runtime::Runtime>> =
    LazyLock::new(|| tokio::runtime::Runtime::new().context("failed to create tokio runtime"));

pub fn get_tokio_handle_result() -> Result<tokio::runtime::Handle> {
    TOKIO_RUNTIME
        .as_ref()
        .map(|rt| rt.handle().clone())
        .map_err(|e| anyhow::anyhow!(e).context("failed to create tokio handle"))
}

pub fn get_tokio_handle() -> tokio::runtime::Handle {
    get_tokio_handle_result().expect("failed to create tokio handle")
}

#[derive(Debug, Clone, clap::Parser)]
#[command(author, version, about)]
pub struct Args {
    #[command(subcommand)]
    command: Option<command::Command>,
}

pub fn main() -> Result<()> {
    let _logging_guard = logging::setup();
    let handle = get_tokio_handle_result()?;
    handle.block_on(async { main_async().await })
}

pub async fn main_async() -> Result<()> {
    std::process::exit(run().await?)
}

pub async fn run() -> Result<i32> {
    let args = Args::parse();

    let (figment, config) = config::get()?;

    if let Some(command) = args.command {
        if let Some(code) = command::run_command(command, &config).await? {
            return Ok(code);
        }
    }

    debug!("Running rocket");
    run_rocket(figment, config).await?;
    debug!("Finished running rocket");
    Ok(0)
}

pub fn get_rocket_build(
    figment: rocket::figment::Figment,
    config: config::Config,
) -> rocket::Rocket<rocket::Build> {
    debug!("Building rocket");
    let rocket = rocket::build()
        .configure(figment)
        .mount("/", rocket::routes![index::index, index::apply_config])
        .mount("/static", FileServer::from("static"))
        .manage(config)
        .attach(Template::fairing());
    debug!("Built rocket");
    rocket
}

pub async fn get_rocket_ignited(
    figment: rocket::figment::Figment,
    config: config::Config,
) -> Result<rocket::Rocket<rocket::Ignite>, anyhow::Error> {
    ignite_rocket(get_rocket_build(figment, config)).await
}

pub async fn get_rocket_launched(
    figment: rocket::figment::Figment,
    config: config::Config,
) -> Result<rocket::Rocket<rocket::Ignite>, anyhow::Error> {
    launch_rocket(get_rocket_ignited(figment, config).await?).await
}

pub async fn ignite_rocket(
    rocket: rocket::Rocket<rocket::Build>,
) -> Result<rocket::Rocket<rocket::Ignite>, anyhow::Error> {
    rocket.ignite().await.context("failed to ignite rocket")
}

pub async fn launch_rocket<P: rocket::Phase>(
    rocket: rocket::Rocket<P>,
) -> Result<rocket::Rocket<rocket::Ignite>, anyhow::Error> {
    rocket.launch().await.context("failed to launch rocket")
}

pub async fn run_rocket(
    figment: rocket::figment::Figment,
    config: config::Config,
) -> Result<rocket::Rocket<rocket::Ignite>, anyhow::Error> {
    get_rocket_launched(figment, config).await
}
