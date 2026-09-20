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
        .map(|p| {
            let result = if p.exists() {
                trash_one(p).map(|_| ())
            } else {
                Err(format!("path no longer exists: {}", p.display()))
            };
            (p.clone(), result)
        })
        .collect()
}

pub fn trash_clean_items(cleaner: &str, items: &[CleanItem]) -> Vec<(PathBuf, Result<(), String>)> {
    trash_clean_items_with_session(cleaner, items)
        .outcomes
        .into_iter()
        .map(|outcome| {
            let result = match outcome.error {
                Some(error) => Err(error),
                None if outcome.moved => Ok(()),
                None => Err("item was not moved to Trash".into()),
            };
            (outcome.path, result)
        })
        .collect()
}

pub struct TrashCleanResult {
    pub session_id: String,
    pub outcomes: Vec<TrashItemResult>,
    pub receipt_error: Option<String>,
}

pub struct TrashItemResult {
    pub path: PathBuf,
    pub moved: bool,
    pub trash_path: Option<PathBuf>,
    pub error: Option<String>,
}

/// Performs a Trash-backed cleanup and returns the durable session identifier
/// used for its receipt and History records.
pub fn trash_clean_items_with_session(cleaner: &str, items: &[CleanItem]) -> TrashCleanResult {
    let session_id = history::new_session_id(cleaner);
    let mut outcomes = Vec::new();
    let mut receipt_items = Vec::new();

    for item in items {
        let path = item.path.clone();
        let preflight_error = if !item.removable {
            Some("item is not marked removable".to_string())
        } else if !path.exists() {
            Some(format!("path no longer exists: {}", path.display()))
        } else {
            safety::validate_removal(&path).err()
        };
        if let Some(error) = preflight_error {
            receipt_items.push(receipt_item(item, "trash", "failed", Some(&error), None));
            outcomes.push(TrashItemResult {
                path,
                moved: false,
                trash_path: None,
                error: Some(error),
            });
            continue;
        }

        match trash_one(&path) {
            Err(error) => {
                receipt_items.push(receipt_item(item, "trash", "failed", Some(&error), None));
                outcomes.push(TrashItemResult {
                    path,
                    moved: false,
                    trash_path: None,
                    error: Some(error),
                });
            }
            Ok(trash_path) => {
                let record = history::HistoryRecord::new(
                    &session_id,
                    cleaner,
                    &item.label,
                    path.clone(),
                    trash_path.clone(),
                    item.size_bytes,
                    "trash",
                );
                let history_error = history::append(&record)
                    .err()
                    .map(|error| format!("moved to Trash but failed to record history: {}", error));
                let status = if history_error.is_some() {
                    "history_failed"
                } else {
                    "moved"
                };
                receipt_items.push(receipt_item(
                    item,
                    "trash",
                    status,
                    history_error.as_deref(),
                    trash_path.as_deref(),
                ));
                outcomes.push(TrashItemResult {
                    path,
                    moved: true,
                    trash_path,
                    error: history_error,
                });
            }
        }
    }

    let receipt_error = if receipt_items.is_empty() {
        None
    } else {
        history::write_receipt(&session_id, cleaner, "trash", receipt_items)
            .err()
            .map(|error| format!("failed to write cleanup receipt: {}", error))
    };

    TrashCleanResult {
        session_id,
        outcomes,
        receipt_error,
    }
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
