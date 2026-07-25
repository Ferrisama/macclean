use crate::core::{AnalysisResult, CleanItem, CleanKind, Cleaner, RiskLevel};
use crate::ui;
use anyhow::Result;
use std::collections::HashMap;
use std::path::PathBuf;

pub struct FontsCleaner;

impl Cleaner for FontsCleaner {
    fn name(&self) -> &str {
        "fonts"
    }
    fn display_name(&self) -> &str {
        "Duplicate Fonts"
    }

    fn analyze(&self) -> Result<AnalysisResult> {
        let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("/tmp"));
        let mut result = AnalysisResult::default();

        let font_dirs = [
            home.join("Library/Fonts"),
            PathBuf::from("/Library/Fonts"),
            PathBuf::from("/System/Library/Fonts"),
        ];

        let font_extensions = ["ttf", "otf", "ttc", "dfont"];

        // Map from lowercase filename -> list of full paths
        let mut filename_map: HashMap<String, Vec<PathBuf>> = HashMap::new();

        for dir in &font_dirs {
            if !dir.exists() {
                continue;
            }
            let entries = match std::fs::read_dir(dir) {
                Ok(e) => e,
                Err(_) => continue,
            };
            for entry in entries.filter_map(|e| e.ok()) {
                let path = entry.path();
                if !path.is_file() {
                    continue;
                }
                let ext = path
                    .extension()
                    .map(|e| e.to_string_lossy().to_lowercase())
                    .unwrap_or_default();
                if font_extensions.contains(&ext.as_str()) {
                    let name = path
                        .file_name()
                        .map(|n| n.to_string_lossy().to_lowercase())
                        .unwrap_or_default();
                    filename_map.entry(name).or_default().push(path);
                }
            }
        }

        // For duplicates, add only user-installed copies (those under ~/Library/Fonts)
        let user_font_dir = home.join("Library/Fonts");
        for paths in filename_map.values() {
            if paths.len() <= 1 {
                continue;
            }
            for path in paths {
                if path.starts_with(&user_font_dir) {
                    let parent = path
                        .parent()
                        .map(|p| p.display().to_string())
                        .unwrap_or_default();
                    let file_name = path
                        .file_name()
                        .map(|n| n.to_string_lossy().to_string())
                        .unwrap_or_default();
                    let size = path.metadata().map(|m| m.len()).unwrap_or(0);
                    result.items.push(CleanItem {
                        label: format!("{} (duplicate in {})", file_name, parent),
                        path: path.clone(),
                        size_bytes: size,
                        removable: true,
                        kind: CleanKind::Duplicate,
                        risk: RiskLevel::Medium,
                        reason: "User font has the same filename as another installed font.".into(),
                    });
                }
            }
        }

        Ok(result)
    }

    fn clean(&self, result: &AnalysisResult, dry_run: bool, yes: bool) -> Result<()> {
        if result.items.is_empty() {
            println!("No duplicate user fonts found.");
            return Ok(());
        }
        ui::print_analysis("Duplicate Fonts", &result.items);
        ui::print_warn("Only user-installed duplicates shown. System fonts will not be touched.");
        if dry_run {
            return Ok(());
        }
        if !yes && !ui::confirm("Remove duplicate user fonts?", false)? {
            return Ok(());
        }

        for (path, outcome) in crate::core::trash::trash_clean_items("fonts", &result.items) {
            match outcome {
                Ok(_) => ui::print_ok(&format!("Moved to Trash: {}", path.display())),
                Err(e) => ui::print_warn(&format!("{}: {}", path.display(), e)),
            }
        }
        Ok(())
    }
}
