use std::path::PathBuf;
use anyhow::Result;
use crate::core::{AnalysisResult, Cleaner};
use crate::core::fs::dir_size;
use crate::ui;

pub struct IosBackupsCleaner;

impl Cleaner for IosBackupsCleaner {
    fn name(&self) -> &str { "ios-backups" }
    fn display_name(&self) -> &str { "iOS Backups" }

    fn analyze(&self) -> Result<AnalysisResult> {
        let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("/tmp"));
        let mut result = AnalysisResult::default();

        let backup_root = home.join("Library/Application Support/MobileSync/Backup");
        if !backup_root.exists() {
            return Ok(result);
        }

        let entries = match std::fs::read_dir(&backup_root) {
            Ok(e) => e,
            Err(_) => return Ok(result),
        };

        for entry in entries.filter_map(|e| e.ok()) {
            let path = entry.path();
            if !path.is_dir() { continue; }

            // Try to get a friendly name from Info.plist; fall back to dir name
            let label = try_read_device_name(&path)
                .unwrap_or_else(|| {
                    path.file_name()
                        .map(|n| n.to_string_lossy().to_string())
                        .unwrap_or_else(|| "Unknown Backup".to_string())
                });

            let size = dir_size(&path);
            result.add(label, path, size);
        }

        Ok(result)
    }

    fn clean(&self, result: &AnalysisResult, dry_run: bool, yes: bool) -> Result<()> {
        if result.items.is_empty() {
            println!("No iOS backups found.");
            return Ok(());
        }
        ui::print_analysis("iOS Backups", &result.items);
        ui::print_warn("Deleting removes only the local backup copy.");
        if dry_run { return Ok(()); }
        if !yes && !ui::confirm("Delete all listed backups?", false)? { return Ok(()); }

        for item in &result.items {
            match std::fs::remove_dir_all(&item.path) {
                Ok(_) => ui::print_ok(&format!("Deleted backup: {}", item.label)),
                Err(e) => ui::print_warn(&format!("{}: {}", item.label, e)),
            }
        }
        Ok(())
    }
}

/// Attempt to read the device name from Info.plist (XML format only).
/// Returns None if the plist is binary or the key is not found.
fn try_read_device_name(backup_dir: &std::path::Path) -> Option<String> {
    let plist_path = backup_dir.join("Info.plist");
    let contents = std::fs::read_to_string(&plist_path).ok()?;

    // Only handle XML plists (binary starts with "bplist")
    if contents.starts_with("bplist") {
        return None;
    }

    // Look for <key>Device Name</key> followed by <string>...</string>
    let key_marker = "<key>Device Name</key>";
    let key_pos = contents.find(key_marker)?;
    let after_key = &contents[key_pos + key_marker.len()..];

    let string_start = after_key.find("<string>")? + "<string>".len();
    let string_end = after_key[string_start..].find("</string>")?;
    let device_name = after_key[string_start..string_start + string_end].trim().to_string();

    if device_name.is_empty() { None } else { Some(device_name) }
}
