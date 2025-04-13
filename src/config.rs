use std::{ffi::OsStr, path::PathBuf};

use anyhow::{Context, Result};
use lexical_sort::PathSort;
use serde::{Deserialize, Serialize};
use tokio::io::{AsyncReadExt, AsyncWriteExt};

use crate::{
    display::DisplayConfig,
    windows_util::{DisplayQueryType, WindowsDisplayConfig},
};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoredConfig {
    pub id: String,
    pub name: String,
    pub display_config: DisplayConfig,
}

pub async fn store_monitor_config(id: &str, name: &str) -> Result<()> {
    let windows_display_config = WindowsDisplayConfig::get(DisplayQueryType::All)?;

    let display_config = DisplayConfig::from_windows(&windows_display_config)?;
    let stored_config = StoredConfig {
        id: id.to_string(),
        name: name.to_string(),
        display_config,
    };

    let json = serde_json::to_string_pretty(&stored_config)?;

    let path = get_display_config_path(id);
    if let Some(parent) = path.parent() {
        tokio::fs::create_dir_all(parent).await?;
    }
    if tokio::fs::try_exists(&path).await? {
        println!("Overwriting monitor config {} \"{}\".", id, name);
    }
    let mut file = tokio::fs::File::create(path).await?;
    file.write_all(json.as_bytes()).await?;
    Ok(())
}

pub async fn load_monitor_config(id: &str) -> Result<Option<StoredConfig>> {
    let path = get_display_config_path(id);
    let path_ref = &path;
    if !tokio::fs::try_exists(path_ref).await? {
        return Ok(None);
    }

    let mut file = tokio::fs::File::open(path_ref).await?;
    let mut bytes = Vec::with_capacity(file.metadata().await?.len() as usize);
    file.read_to_end(&mut bytes)
        .await
        .with_context(|| format!("Failed to read monitor config at {}", path_ref.display()))?;
    let json = String::from_utf8(bytes).with_context(|| {
        format!(
            "Failed to parse monitor config as UTF-8 at {}",
            path_ref.display()
        )
    })?;
    Ok(Some(serde_json::from_str(&json).with_context(|| {
        format!("Failed to parse monitor config at {}", path_ref.display())
    })?))
}

pub async fn get_all_display_configs() -> Result<Vec<StoredConfig>> {
    let paths = get_all_display_config_paths().await?;
    let mut configs = Vec::with_capacity(paths.len());
    // println!("paths: {:?}", paths);
    for path in paths {
        // println!(" path: {:?}", path);
        let mut file = tokio::fs::File::open(path).await?;
        let mut contents = String::new();
        file.read_to_string(&mut contents).await?;
        let config: StoredConfig = serde_json::from_str(&contents)?;
        // println!(" pushing config: {:?}", config.id);
        configs.push(config);
    }
    // println!("config IDs: {:?}", configs.iter().map(|c| c.id.clone()).collect::<Vec<_>>());
    Ok(configs)
}

fn get_display_config_directory() -> PathBuf {
    PathBuf::from("display_config")
}

fn get_display_config_path(id: &str) -> PathBuf {
    get_display_config_directory().join(format!("{}.json", id))
}

async fn get_all_display_config_paths() -> Result<Vec<PathBuf>> {
    let mut paths = Vec::new();
    let mut entries = tokio::fs::read_dir(get_display_config_directory()).await?;
    while let Some(entry) = entries.next_entry().await? {
        let path = entry.path();
        if path.is_file() && path.extension() == Some(OsStr::new("json")) {
            paths.push(path);
        }
    }
    paths.path_sort(lexical_sort::natural_lexical_cmp);
    Ok(paths)
}
