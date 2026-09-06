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
        .map(|p| (p.clone(), trash_one(p).map(|_| ())))
        .collect()
}

pub fn trash_clean_items(cleaner: &str, items: &[CleanItem]) -> Vec<(PathBuf, Result<(), String>)> {
    let session_id = history::new_session_id(cleaner);
    let mut outcomes = Vec::new();
    let mut receipt_items = Vec::new();

    items
        .iter()
        .filter(|item| item.removable && item.path.exists())
        .for_each(|item| {
            let path = item.path.clone();
            if let Err(e) = safety::validate_removal(&path) {
                receipt_items.push(receipt_item(item, "trash", "failed", Some(&e), None));
                outcomes.push((path, Err(e)));
                return;
            }
            let result = trash_one(&path);
            if let Ok(trash_path) = &result {
                let record = history::HistoryRecord::new(
                    &session_id,
                    cleaner,
                    &item.label,
                    path.clone(),
                    trash_path.clone(),
                    item.size_bytes,
                    "trash",
                );
                if let Err(e) = history::append(&record) {
                    let err = format!("moved to Trash but failed to record history: {}", e);
                    receipt_items.push(receipt_item(
                        item,
                        "trash",
                        "history_failed",
                        Some(&err),
                        trash_path.as_deref(),
                    ));
                    outcomes.push((path, Err(err)));
                    return;
                }
            }
            match &result {
                Ok(trash_path) => receipt_items.push(receipt_item(
                    item,
                    "trash",
                    "moved",
                    None,
                    trash_path.as_deref(),
                )),
                Err(e) => receipt_items.push(receipt_item(item, "trash", "failed", Some(e), None)),
            }
            outcomes.push((path, result.map(|_| ())));
        });

    if !receipt_items.is_empty() {
        let _ = history::write_receipt(&session_id, cleaner, "trash", receipt_items);
    }

    outcomes
}

#[cfg(target_os = "macos")]
fn trash_one(path: &Path) -> Result<Option<PathBuf>, String> {
    use objc2_foundation::{NSFileManager, NSString, NSURL};

    let path_text = path
        .to_str()
        .ok_or_else(|| format!("cleanup path is not valid UTF-8: {}", path.display()))?;
    let source_url = NSURL::fileURLWithPath(&NSString::from_str(path_text));
    let mut resulting_url = None;
    NSFileManager::defaultManager()
        .trashItemAtURL_resultingItemURL_error(&source_url, Some(&mut resulting_url))
        .map_err(|error| {
            format!(
                "macOS could not move {} to Trash: {}",
                path.display(),
                error
            )
        })?;
    let destination = resulting_url
        .and_then(|url| url.path())
        .map(|path| PathBuf::from(path.to_string()))
        .ok_or_else(|| {
            "macOS moved the item but did not return its Trash destination".to_string()
        })?;
    Ok(Some(destination))
}

#[cfg(not(target_os = "macos"))]
fn trash_one(path: &Path) -> Result<Option<PathBuf>, String> {
    trash::delete(path).map_err(|e| e.to_string())?;
    Ok(None)
}

fn receipt_item(
    item: &CleanItem,
    method: &str,
    status: &str,
    error: Option<&str>,
    trash_path: Option<&Path>,
) -> history::ReceiptItem {
    let restore = if trash_path.is_some() {
        "available after Trash move"
    } else {
        "not available"
    };
    history::ReceiptItem {
        label: item.label.clone(),
        path: item.path.clone(),
        size_bytes: item.size_bytes,
        method: method.into(),
        status: status.into(),
        restore: restore.into(),
        error: error.map(str::to_string),
        trash_path: trash_path.map(Path::to_path_buf),
    }
}
