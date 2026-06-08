use anyhow::{bail, Result};
use colored::Colorize;
use comfy_table::{Table, presets::UTF8_BORDERS_ONLY};
use std::path::{Path, PathBuf};
use crate::core::fs::dir_size;
use crate::ui::{confirm, format_size, print_err, print_ok, print_warn};

/// Search /Applications and ~/Applications for an .app bundle whose
/// directory name matches (case-insensitive) `app_name`.
fn find_app(app_name: &str) -> Option<PathBuf> {
    let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
    let search_dirs = [
        PathBuf::from("/Applications"),
        home.join("Applications"),
    ];

    let lower = app_name.to_lowercase();

    for dir in &search_dirs {
        if !dir.exists() {
            continue;
        }
        let Ok(entries) = std::fs::read_dir(dir) else { continue };
        for entry in entries.filter_map(|e| e.ok()) {
            let fname = entry.file_name();
            let name  = fname.to_string_lossy().to_lowercase();
            if name == format!("{}.app", lower)
                || name == lower
                || name == format!("{}.app", lower.replace(' ', ""))
            {
                return Some(entry.path());
            }
        }
    }
    None
}

/// Read XML plist and extract CFBundleIdentifier value.
fn bundle_id_from_plist(app_path: &Path) -> Option<String> {
    let plist_path = app_path.join("Contents/Info.plist");
    let xml = std::fs::read_to_string(&plist_path).ok()?;

    // Skip binary plists
    if xml.trim_start().starts_with("bplist") {
        return None;
    }

    let key_tag = "<key>CFBundleIdentifier</key>";
    let pos     = xml.find(key_tag)?;
    let rest    = &xml[pos + key_tag.len()..];
    let start   = rest.find("<string>")? + "<string>".len();
    let end     = rest[start..].find("</string>")?;
    Some(rest[start..start + end].trim().to_string())
}

/// Scan a directory for sub-paths that match the bundle_id or app_name.
fn find_traces(
    search_dir: &Path,
    bundle_id: &str,
    app_name: &str,
) -> Vec<PathBuf> {
    if !search_dir.exists() {
        return Vec::new();
    }
    let Ok(entries) = std::fs::read_dir(search_dir) else { return Vec::new() };

    let bid_lower  = bundle_id.to_lowercase();
    let name_lower = app_name.to_lowercase();

    entries
        .filter_map(|e| e.ok())
        .filter(|e| {
            let fname = e.file_name();
            let n = fname.to_string_lossy().to_lowercase();
            n.contains(&bid_lower) || n.contains(&name_lower)
        })
        .map(|e| e.path())
        .collect()
}

pub fn run(app_name: &str, dry_run: bool, yes: bool) -> Result<()> {
    println!("\n{}", format!("[ Uninstall: {} ]", app_name).cyan().bold());

    // ── Find the app bundle ───────────────────────────────────────────────────
    let app_path = match find_app(app_name) {
        Some(p) => p,
        None    => bail!("Could not find '{}' in /Applications or ~/Applications.", app_name),
    };
    println!("  Found: {}", app_path.display());

    let bundle_id = bundle_id_from_plist(&app_path)
        .unwrap_or_else(|| app_name.to_lowercase().replace(' ', "."));
    println!("  Bundle ID: {}", bundle_id);

    // ── Locate traces ─────────────────────────────────────────────────────────
    let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));

    let scan_dirs = [
        home.join("Library/Application Support"),
        home.join("Library/Caches"),
        home.join("Library/Containers"),
        home.join("Library/Preferences"),
        home.join("Library/Logs"),
        home.join("Library/Saved Application State"),
    ];

    let mut to_remove: Vec<PathBuf> = vec![app_path.clone()];
    for dir in &scan_dirs {
        let mut traces = find_traces(dir, &bundle_id, app_name);
        to_remove.append(&mut traces);
    }

    // Deduplicate
    to_remove.dedup();

    // ── Show table ────────────────────────────────────────────────────────────
    let mut table = Table::new();
    table.load_preset(UTF8_BORDERS_ONLY);
    table.set_header(vec!["Path", "Size"]);

    let mut total_size: u64 = 0;
    for path in &to_remove {
        let size = if path.is_dir() {
            dir_size(path)
        } else {
            path.metadata().map(|m| m.len()).unwrap_or(0)
        };
        total_size += size;

        let label = path
            .strip_prefix(&home)
            .map(|p| format!("~/{}", p.display()))
            .unwrap_or_else(|_| path.display().to_string());

        table.add_row(vec![label, format_size(size)]);
    }
    println!("{}", table);
    println!("  Total: {}", format_size(total_size).bold());

    if dry_run {
        print_warn("Dry run — nothing removed.");
        return Ok(());
    }

    if !yes {
        let prompt = format!("Permanently delete {} and all its files?", app_name);
        if !confirm(&prompt, false)? {
            println!("  Aborted.");
            return Ok(());
        }
    }

    // ── Remove ────────────────────────────────────────────────────────────────
    for path in &to_remove {
        if !path.exists() {
            continue;
        }
        let result = if path.is_dir() {
            std::fs::remove_dir_all(path).map_err(|e| e.to_string())
        } else {
            std::fs::remove_file(path).map_err(|e| e.to_string())
        };

        match result {
            Ok(_)  => print_ok(&format!("Removed: {}", path.display())),
            Err(e) => print_err(&format!("Failed to remove {}: {}", path.display(), e)),
        }
    }

    Ok(())
}
