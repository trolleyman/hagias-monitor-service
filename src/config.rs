use std::path::PathBuf;

use anyhow::{Context, Result};
use derive_more::IntoIterator;
use serde::{Deserialize, Serialize};
use tokio::io::AsyncReadExt;

use crate::{
    display::DisplayLayout,
    windows_util::{DisplayQueryType, WindowsDisplayConfig},
};

#[derive(Debug, Clone, Serialize, Deserialize, Default, IntoIterator)]
#[serde(transparent)]
pub struct Layouts(Vec<NamedLayout>);

impl Layouts {
    pub fn get_path() -> PathBuf {
        PathBuf::from("layouts.json")
    }

    pub fn new() -> Self {
        Self(Vec::new())
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    pub async fn load() -> Result<Self> {
        Self::load_private()
            .await
            .with_context(|| format!("Failed to load layouts at {}", Self::get_path().display()))
    }

    async fn load_private() -> Result<Self> {
        let path = Self::get_path();
        Ok(if !tokio::fs::try_exists(&path).await? {
            Self::new()
        } else {
            let mut file = tokio::fs::File::open(&path).await?;
            let mut bytes = Vec::with_capacity(file.metadata().await?.len() as usize);
            file.read_to_end(&mut bytes).await?;
            let json = String::from_utf8(bytes).context("Invalid UTF-8")?;
            serde_json::from_str(&json).context("Invalid JSON")?
        })
    }

    pub async fn save(&self) -> Result<()> {
        self.save_private()
            .await
            .with_context(|| format!("Failed to save layouts at {}", Self::get_path().display()))
    }

    async fn save_private(&self) -> Result<()> {
        let path = Self::get_path();
        let json = serde_json::to_string_pretty(self)?;
        tokio::fs::write(path, json).await?;
        Ok(())
    }

    pub async fn add_current(&mut self, id: &str, name: &str) -> Result<()> {
        let windows_display_config = WindowsDisplayConfig::get(DisplayQueryType::All)?;
        let layout = DisplayLayout::from_windows(&windows_display_config)?;
        let named_layout = NamedLayout {
            id: id.into(),
            name: name.into(),
            layout,
        };
        self.add_layout(named_layout);
        Ok(())
    }

    pub fn add_layout(&mut self, layout: NamedLayout) {
        self.0.push(layout);
    }

    pub fn remove_layout(&mut self, id: &str) -> Option<NamedLayout> {
        let index = self.0.iter().position(|l| l.id == id)?;
        Some(self.0.remove(index))
    }

    pub fn get_layout(&self, id: &str) -> Option<&NamedLayout> {
        self.0.iter().find(|l| l.id == id)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NamedLayout {
    pub id: String,
    pub name: String,
    pub layout: DisplayLayout,
}
