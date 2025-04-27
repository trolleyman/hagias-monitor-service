use anyhow::Context as _;
use rocket::figment::{
    providers::{Format, Toml},
    value::magic::RelativePathBuf,
};
use serde::Deserialize;
use tracing::debug;

#[derive(Debug, Clone, Deserialize)]
pub struct Config {
    pub layouts_path: RelativePathBuf,
    pub static_dir: RelativePathBuf,
    pub template_dir: RelativePathBuf,
}

pub fn get() -> Result<(rocket::figment::Figment, Config), anyhow::Error> {
    debug!("Loading config...");
    let mut figment = rocket::Config::figment();
    if let Some(rocket_toml_path) = std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(|p| p.join("Rocket.toml")))
    {
        figment = figment.merge(Toml::file(rocket_toml_path).nested());
    }
    let config = figment
        .extract::<Config>()
        .context("Failed to extract config")?;
    debug!("Loaded config");
    debug!(
        "  layouts_path: {}",
        config.layouts_path.relative().display()
    );
    debug!("  static_dir: {}", config.static_dir.relative().display());
    debug!(
        "  template_dir: {}",
        config.template_dir.relative().display()
    );
    Ok((figment, config))
}
