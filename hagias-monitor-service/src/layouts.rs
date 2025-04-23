use std::path::Path;

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
    pub fn new() -> Self {
        Self(Vec::new())
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    pub fn iter(&self) -> std::slice::Iter<'_, NamedLayout> {
        self.0.iter()
    }

    pub fn swap(&mut self, a: usize, b: usize) {
        self.0.swap(a, b);
    }

    pub async fn load(layouts_path: &Path) -> Result<Self> {
        eprintln!("Loading layouts from {}", layouts_path.display());
        Self::load_private(layouts_path)
            .await
            .with_context(|| format!("Failed to load layouts at {}", layouts_path.display()))
    }

    async fn load_private(layouts_path: &Path) -> Result<Self> {
        Ok(if !tokio::fs::try_exists(layouts_path).await? {
            Self::new()
        } else {
            let mut file = tokio::fs::File::open(layouts_path).await?;
            let mut bytes = Vec::with_capacity(file.metadata().await?.len() as usize);
            file.read_to_end(&mut bytes).await?;
            let json = String::from_utf8(bytes).context("Invalid UTF-8")?;
            serde_json::from_str(&json).context("Invalid JSON")?
        })
    }

    pub async fn save(&self, layouts_path: &Path) -> Result<()> {
        eprintln!("Saving layouts to {}", layouts_path.display());
        self.save_private(layouts_path)
            .await
            .with_context(|| format!("Failed to save layouts at {}", layouts_path.display()))
    }

    async fn save_private(&self, layouts_path: &Path) -> Result<()> {
        let json = serde_json::to_string_pretty(self)?;
        tokio::fs::write(layouts_path, json).await?;
        Ok(())
    }

    pub async fn add_current(&mut self, id: &str, name: &str, emoji: Option<&str>) -> Result<()> {
        let windows_display_config = WindowsDisplayConfig::get(DisplayQueryType::All)?;
        let layout = DisplayLayout::from_windows(&windows_display_config)?;
        let named_layout = NamedLayout {
            id: id.into(),
            name: name.into(),
            emoji: emoji.map(|s| s.into()),
            hidden: false,
            layout,
        };
        self.add_layout(named_layout);
        Ok(())
    }

    pub fn add_layout(&mut self, layout: NamedLayout) {
        self.0.retain(|l| l.id != layout.id);
        self.0.push(layout);
    }

    pub fn remove_layout(&mut self, id: &str) -> Option<NamedLayout> {
        let index = self.0.iter().position(|l| l.id == id)?;
        Some(self.0.remove(index))
    }

    pub fn get_layout(&self, id: &str) -> Option<&NamedLayout> {
        self.0.iter().find(|l| l.id == id)
    }

    pub fn get_layout_mut(&mut self, id: &str) -> Option<&mut NamedLayout> {
        self.0.iter_mut().find(|l| l.id == id)
    }
}

impl std::ops::Index<usize> for Layouts {
    type Output = NamedLayout;

    fn index(&self, index: usize) -> &Self::Output {
        &self.0[index]
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NamedLayout {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub emoji: Option<String>,
    #[serde(default)]
    pub hidden: bool,
    pub layout: DisplayLayout,
}
