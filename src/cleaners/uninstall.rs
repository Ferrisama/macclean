use anyhow::{bail, Result};
use colored::Colorize;
use comfy_table::{presets::UTF8_BORDERS_ONLY, Table};
use std::path::{Path, PathBuf};
use crate::core::fs::dir_size;
use crate::ui::{confirm, format_size, print_err, print_ok, print_warn};

/// One path found to belong to an app, with its size on disk.
pub struct TraceItem {
    pub path: PathBuf,
    pub size_bytes: u64,
}

/// The result of scanning for an app and everything associated with it,
/// built once and shared between the CLI and TUI front ends.
pub struct UninstallPlan {
    pub app_name: String,
    pub bundle_id: String,
    pub bundle_id_guessed: bool,
    pub items: Vec<TraceItem>,
    pub total_size: u64,
}

/// List installed .app bundles in /Applications and ~/Applications as
/// (display name, path) pairs, for an app picker.
pub fn list_installed_apps() -> Vec<(String, PathBuf)> {
    let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
    let mut apps = Vec::new();
    for dir in [PathBuf::from("/Applications"), home.join("Applications")] {
        let Ok(entries) = std::fs::read_dir(&dir) else { continue };
        for entry in entries.filter_map(|e| e.ok()) {
            let name = entry.file_name().to_string_lossy().to_string();
            if let Some(stripped) = name.strip_suffix(".app") {
                apps.push((stripped.to_string(), entry.path()));
            }
        }
    }
    apps.sort_by_key(|a| a.0.to_lowercase());
    apps.dedup_by(|a, b| a.0.eq_ignore_ascii_case(&b.0));
    apps
}

/// Search /Applications and ~/Applications for an .app bundle whose
/// directory name matches (case-insensitive) `app_name`.
fn find_app(app_name: &str) -> Option<PathBuf> {
    let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
    let search_dirs = [PathBuf::from("/Applications"), home.join("Applications")];

    let lower = app_name.to_lowercase();

    for dir in &search_dirs {
        if !dir.exists() {
            continue;
        }
        let Ok(entries) = std::fs::read_dir(dir) else { continue };
        for entry in entries.filter_map(|e| e.ok()) {
            let fname = entry.file_name();
            let name = fname.to_string_lossy().to_lowercase();
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

/// Scan a directory for sub-paths that match the bundle_id or app_name.
fn find_traces(search_dir: &Path, bundle_id: &str, app_name: &str) -> Vec<PathBuf> {
    if !search_dir.exists() {
        return Vec::new();
    }
    let Ok(entries) = std::fs::read_dir(search_dir) else { return Vec::new() };

    let bid_lower = bundle_id.to_lowercase();
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

/// Find an app and everything associated with it, without printing or
/// touching anything. Shared by the CLI's `uninstall` command and the TUI's
/// Uninstall tab.
pub fn build_plan(app_name: &str) -> Result<UninstallPlan> {
    let app_path = match find_app(app_name) {
        Some(p) => p,
        None => bail!("Could not find '{}' in /Applications or ~/Applications.", app_name),
    };

    let (bundle_id, bundle_id_guessed) = match crate::core::plist::read_bundle_id(&app_path) {
        Some(id) => (id, false),
        None => (app_name.to_lowercase().replace(' ', "."), true),
    };

    let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
    let scan_dirs = [
        home.join("Library/Application Support"),
        home.join("Library/Caches"),
        home.join("Library/Containers"),
        home.join("Library/Preferences"),
        home.join("Library/Logs"),
        home.join("Library/Saved Application State"),
    ];

    let mut to_remove: Vec<PathBuf> = vec![app_path];
    for dir in &scan_dirs {
        to_remove.append(&mut find_traces(dir, &bundle_id, app_name));
    }
    to_remove.dedup();

    let mut total_size = 0u64;
    let items: Vec<TraceItem> = to_remove
        .into_iter()
        .map(|path| {
            let size_bytes = if path.is_dir() {
                dir_size(&path)
            } else {
                path.metadata().map(|m| m.len()).unwrap_or(0)
            };
            total_size += size_bytes;
            TraceItem { path, size_bytes }
        })
        .collect();

    Ok(UninstallPlan {
        app_name: app_name.to_string(),
        bundle_id,
        bundle_id_guessed,
        items,
        total_size,
    })
}

/// Move everything in the plan to the Trash (recoverable). Returns one
/// result per item.
pub fn execute(plan: &UninstallPlan) -> Vec<(PathBuf, std::result::Result<(), String>)> {
    let paths: Vec<PathBuf> = plan.items.iter().map(|item| item.path.clone()).collect();
    crate::core::trash::trash_paths(&paths)
}

pub fn run(app_name: &str, dry_run: bool, yes: bool) -> Result<()> {
    println!("\n{}", format!("[ Uninstall: {} ]", app_name).cyan().bold());

    let plan = build_plan(app_name)?;
    println!("  Found: {}", plan.items[0].path.display());
    if plan.bundle_id_guessed {
        print_warn(&format!(
            "Could not read Info.plist -- guessing bundle ID '{}'. Review the paths below carefully.",
            plan.bundle_id
        ));
    }
    println!("  Bundle ID: {}", plan.bundle_id);

    let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
    let mut table = Table::new();
    table.load_preset(UTF8_BORDERS_ONLY);
    table.set_header(vec!["Path", "Size"]);
    for item in &plan.items {
        let label = item
            .path
            .strip_prefix(&home)
            .map(|p| format!("~/{}", p.display()))
            .unwrap_or_else(|_| item.path.display().to_string());
        table.add_row(vec![label, format_size(item.size_bytes)]);
    }
    println!("{}", table);
    println!("  Total: {}", format_size(plan.total_size).bold());

    if dry_run {
        print_warn("Dry run — nothing removed.");
        return Ok(());
    }

    if !yes {
        let prompt = format!("Move {} and all its files to the Trash?", app_name);
        if !confirm(&prompt, false)? {
            println!("  Aborted.");
            return Ok(());
        }
    }

    for (path, result) in execute(&plan) {
        match result {
            Ok(_) => print_ok(&format!("Moved to Trash: {}", path.display())),
            Err(e) => print_err(&format!("Failed to trash {}: {}", path.display(), e)),
        }
    }

    Ok(())
}
