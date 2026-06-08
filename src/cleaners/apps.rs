use std::collections::HashSet;
use std::path::PathBuf;
use anyhow::Result;
use crate::core::{AnalysisResult, Cleaner};
use crate::core::fs::dir_size;
use crate::ui;

pub struct AppsCleaner;

// Prefixes for app support dirs to always skip (system / well-known apps)
const KEEP_PREFIXES: &[&str] = &[
    "com.apple.",
    "com.google.",
    "io.iterm2",
    "com.microsoft.",
];

impl Cleaner for AppsCleaner {
    fn name(&self) -> &str { "apps" }
    fn display_name(&self) -> &str { "Orphaned App Support" }

    fn analyze(&self) -> Result<AnalysisResult> {
        let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("/tmp"));
        let mut result = AnalysisResult::default();

        // Collect installed app names (without .app) from /Applications and ~/Applications
        let mut installed: HashSet<String> = HashSet::new();
        for app_root in &[PathBuf::from("/Applications"), home.join("Applications")] {
            if !app_root.exists() { continue; }
            if let Ok(entries) = std::fs::read_dir(app_root) {
                for entry in entries.filter_map(|e| e.ok()) {
                    let name = entry.file_name().to_string_lossy().to_string();
                    if name.ends_with(".app") {
                        let app_name = name.trim_end_matches(".app").to_lowercase();
                        installed.insert(app_name.clone());

                        // Try to extract bundle ID from Info.plist
                        let plist = entry.path().join("Contents/Info.plist");
                        if let Some(bundle_id) = read_bundle_id(&plist) {
                            installed.insert(bundle_id.to_lowercase());
                        }
                    }
                }
            }
        }

        // Scan support directories for orphaned entries
        let scan_dirs = [
            home.join("Library/Application Support"),
            home.join("Library/Containers"),
            home.join("Library/Preferences"),
        ];

        for scan_dir in &scan_dirs {
            if !scan_dir.exists() { continue; }
            let entries = match std::fs::read_dir(scan_dir) {
                Ok(e) => e,
                Err(_) => continue,
            };
            for entry in entries.filter_map(|e| e.ok()) {
                let path = entry.path();
                let name = entry.file_name().to_string_lossy().to_string();

                // Only consider entries that look like bundle IDs (contain at least one dot)
                if !name.contains('.') { continue; }
                // Must be alphanumeric + dots + hyphens
                if !name.chars().all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '-' || c == '_') {
                    continue;
                }

                // Skip well-known system/app prefixes
                if KEEP_PREFIXES.iter().any(|p| name.starts_with(p)) { continue; }

                // Skip if any installed app name or bundle ID matches
                let name_lower = name.to_lowercase();
                if installed.iter().any(|inst| {
                    name_lower.contains(inst.as_str()) || inst.contains(name_lower.as_str())
                }) {
                    continue;
                }

                if path.is_dir() {
                    let size = dir_size(&path);
                    result.add(name, path, size);
                } else if path.is_file() {
                    // Preferences .plist files
                    let size = path.metadata().map(|m| m.len()).unwrap_or(0);
                    result.add(name, path, size);
                }
            }
        }

        Ok(result)
    }

    fn clean(&self, result: &AnalysisResult, dry_run: bool, yes: bool) -> Result<()> {
        if result.items.is_empty() {
            println!("No orphaned app support folders found.");
            return Ok(());
        }
        ui::print_analysis("Orphaned App Support", &result.items);
        ui::print_warn("Review carefully -- some items may be needed for cloud-synced or sandboxed apps.");
        if dry_run { return Ok(()); }
        if !yes && !ui::confirm("Delete all ghost app folders?", false)? { return Ok(()); }

        for item in &result.items {
            if item.path.is_dir() {
                match std::fs::remove_dir_all(&item.path) {
                    Ok(_) => ui::print_ok(&format!("Removed {}", item.label)),
                    Err(e) => ui::print_warn(&format!("{}: {}", item.label, e)),
                }
            } else {
                match std::fs::remove_file(&item.path) {
                    Ok(_) => ui::print_ok(&format!("Removed {}", item.label)),
                    Err(e) => ui::print_warn(&format!("{}: {}", item.label, e)),
                }
            }
        }
        Ok(())
    }
}

/// Read CFBundleIdentifier from an XML Info.plist. Returns None for binary plists or missing key.
fn read_bundle_id(plist_path: &std::path::Path) -> Option<String> {
    let contents = std::fs::read_to_string(plist_path).ok()?;
    if contents.starts_with("bplist") { return None; }

    let key_marker = "<key>CFBundleIdentifier</key>";
    let key_pos = contents.find(key_marker)?;
    let after = &contents[key_pos + key_marker.len()..];

    let start = after.find("<string>")? + "<string>".len();
    let end = after[start..].find("</string>")?;
    let bundle_id = after[start..start + end].trim().to_string();

    if bundle_id.is_empty() { None } else { Some(bundle_id) }
}
