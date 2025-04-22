use anyhow::{Context, Error, Result};
use ignore::{DirEntry, WalkBuilder, WalkState};
use std::{
    collections::HashSet,
    path::{Path, PathBuf},
    sync::mpsc::{Sender, channel},
};

#[derive(Debug)]
struct GatherResult {
    pub files: HashSet<PathBuf>,
    pub directories: HashSet<PathBuf>,
    pub errors: Vec<Error>,
}

struct FileGathererBuilder {
    result_tx: Sender<GatherResult>,
}

impl<'s> ignore::ParallelVisitorBuilder<'s> for FileGathererBuilder {
    fn build(&mut self) -> Box<dyn ignore::ParallelVisitor + 's> {
        Box::new(FileGatherer {
            result_tx: self.result_tx.clone(),
            files: HashSet::new(),
            directories: HashSet::new(),
            errors: Vec::new(),
        })
    }
}

struct FileGatherer {
    result_tx: Sender<GatherResult>,
    files: HashSet<PathBuf>,
    directories: HashSet<PathBuf>,
    errors: Vec<Error>,
}

// Send results when the visitor is dropped
impl Drop for FileGatherer {
    fn drop(&mut self) {
        let _ = self.result_tx.send(GatherResult {
            files: std::mem::take(&mut self.files),
            directories: std::mem::take(&mut self.directories),
            errors: std::mem::take(&mut self.errors),
        });
    }
}

impl ignore::ParallelVisitor for FileGatherer {
    fn visit(&mut self, result: Result<DirEntry, ignore::Error>) -> WalkState {
        match result {
            Ok(entry) => {
                if let Some(ft) = entry.file_type() {
                    if ft.is_file() {
                        self.files.insert(entry.path().to_path_buf());
                    } else if ft.is_dir() {
                        self.directories.insert(entry.path().to_path_buf());
                    }
                }
            }
            Err(err) => {
                self.errors.push(anyhow::anyhow!(err));
            }
        }
        WalkState::Continue
    }
}

pub fn get_unignored_files_and_directories(
    root: &Path,
) -> Result<(HashSet<PathBuf>, HashSet<PathBuf>)> {
    // Create channel for collecting results
    let (result_tx, result_rx) = channel();

    // Create the walker and visit all entries
    WalkBuilder::new(root)
        .build_parallel()
        .visit(&mut FileGathererBuilder {
            result_tx: result_tx.clone(),
        });

    // Drop the original sender to allow the receiver to know when to stop
    drop(result_tx);

    // Combine all results
    let mut all_files = HashSet::new();
    let mut all_directories = HashSet::new();
    let mut all_errors = Vec::new();

    for result in result_rx {
        all_files.extend(result.files);
        all_directories.extend(result.directories);
        all_errors.extend(result.errors);
    }

    if !all_errors.is_empty() {
        let mut result = Err(anyhow::anyhow!(all_errors.remove(0))).with_context(|| {
            format!(
                "failed to gather files and directories from `{}`",
                root.display()
            )
        });
        if !all_errors.is_empty() {
            result = result.with_context(|| format!("other errors: {:?}", all_errors));
        }
        return result;
    }

    let all_files = all_files
        .iter()
        .map(|f| {
            f.canonicalize()
                .with_context(|| format!("failed to canonicalize path: {}", f.display()))
        })
        .collect::<Result<HashSet<_>>>()?;
    let all_directories = all_directories
        .iter()
        .map(|d| {
            d.canonicalize()
                .with_context(|| format!("failed to canonicalize path: {}", d.display()))
        })
        .collect::<Result<HashSet<_>>>()?;

    Ok((all_files, all_directories))
}
