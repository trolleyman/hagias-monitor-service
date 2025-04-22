use anyhow::{Context, Result};
use std::path::{Path, PathBuf};

use crate::print::print_cargo_style;

/// Canonicalize a path, or return the original path if it fails
pub fn canonicalize_or_original(path: &Path) -> PathBuf {
    std::fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf())
}

/// Normalize a path to be relative to the current directory, to make it easier to read
pub fn normalize_path(path: &Path) -> PathBuf {
    let current_dir = match std::env::current_dir() {
        Ok(dir) => canonicalize_or_original(&dir),
        Err(_) => return path.to_path_buf(),
    };

    let canonical_path = canonicalize_or_original(path);

    if let Ok(relative) = canonical_path.strip_prefix(&current_dir) {
        return relative.to_path_buf();
    }
    if let Ok(relative) = path.strip_prefix(&current_dir) {
        return relative.to_path_buf();
    }
    canonical_path
}

/// Copy a file, printing a message in cargo style
pub fn copy_file(src: &Path, dest: &Path) -> Result<()> {
    let src_normalized = normalize_path(src);
    let dest_normalized = normalize_path(dest);
    let dest_dir = dest_normalized.parent().unwrap_or(&dest_normalized);

    print_cargo_style(
        "Copying",
        &format!("`{}` to `{}`", src_normalized.display(), dest_dir.display()),
    );
    if let Some(dest_parent) = dest_normalized.parent() {
        std::fs::create_dir_all(dest_parent)
            .with_context(|| format!("failed to create directory `{}`", dest_parent.display()))?;
    }
    std::fs::copy(src, dest).with_context(|| {
        format!(
            "failed to copy `{}` to `{}`",
            src_normalized.display(),
            dest_normalized.display()
        )
    })?;
    Ok(())
}

/// Copy a directory, printing a message in cargo style
pub fn copy_dir(src: &Path, dest: &Path) -> Result<()> {
    let src_normalized = normalize_path(src);
    let dest_normalized = normalize_path(dest);
    let dest_dir = dest_normalized.parent().unwrap_or(&dest_normalized);

    print_cargo_style(
        "Copying",
        &format!("`{}` to `{}`", src_normalized.display(), dest_dir.display()),
    );

    copy_dir_silent(src, dest)?;
    Ok(())
}

/// Copy a directory silently, without printing a message in cargo style
pub fn copy_dir_silent(src: &Path, dest: &Path) -> Result<()> {
    std::fs::create_dir_all(dest)?;
    for entry in std::fs::read_dir(src)? {
        let entry = entry?;
        let src_path = entry.path();
        let dest_path = dest.join(src_path.file_name().unwrap());
        if src_path.is_dir() {
            copy_dir_silent(&src_path, &dest_path)?;
        } else {
            std::fs::copy(&src_path, &dest_path).with_context(|| {
                format!(
                    "failed to copy `{}` to `{}`",
                    src_path.display(),
                    dest_path.display()
                )
            })?;
        }
    }
    Ok(())
}
