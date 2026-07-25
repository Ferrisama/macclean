use std::path::{Path, PathBuf};

use crate::core::{history, safety, CleanItem};

/// Move each path to the macOS Trash (recoverable) instead of permanently
/// deleting it. Used where the deleted data is hard to regenerate (an
/// installed app and its settings) -- unlike the cache/log cleaners, where
/// permanent, immediate deletion is the entire point (the data is trivially
/// regenerable, and moving multi-GB caches to Trash wouldn't even free the
/// space until Trash is emptied).
pub fn trash_paths(paths: &[PathBuf]) -> Vec<(PathBuf, Result<(), String>)> {
    paths
        .iter()
        .filter(|p| p.exists())
        .map(|p| (p.clone(), trash_one(p)))
        .collect()
}

pub fn trash_clean_items(cleaner: &str, items: &[CleanItem]) -> Vec<(PathBuf, Result<(), String>)> {
    let session_id = history::new_session_id(cleaner);
    items
        .iter()
        .filter(|item| item.removable && item.path.exists())
        .map(|item| {
            let path = item.path.clone();
            if let Err(e) = safety::validate_removal(&path) {
                return (path, Err(e));
            }
            let result = trash_one(&path);
            if result.is_ok() {
                let record = history::HistoryRecord::new(
                    &session_id,
                    cleaner,
                    &item.label,
                    path.clone(),
                    item.size_bytes,
                    "trash",
                );
                if let Err(e) = history::append(&record) {
                    return (
                        path,
                        Err(format!(
                            "moved to Trash but failed to record history: {}",
                            e
                        )),
                    );
                }
            }
            (path, result)
        })
        .collect()
}

fn trash_one(path: &Path) -> Result<(), String> {
    trash::delete(path).map_err(|e| e.to_string())
}
