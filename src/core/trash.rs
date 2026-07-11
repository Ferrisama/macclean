use std::path::{Path, PathBuf};

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

fn trash_one(path: &Path) -> Result<(), String> {
    trash::delete(path).map_err(|e| e.to_string())
}
