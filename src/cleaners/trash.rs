use std::path::PathBuf;
use anyhow::Result;
use crate::core::{AnalysisResult, Cleaner};
use crate::core::fs::{dir_size, remove_dir_contents};
use crate::ui;

pub struct TrashCleaner;

impl Cleaner for TrashCleaner {
    fn name(&self) -> &str { "trash" }
    fn display_name(&self) -> &str { "Trash" }

    fn analyze(&self) -> Result<AnalysisResult> {
        let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("/tmp"));
        let mut result = AnalysisResult::default();

        let user_trash = home.join(".Trash");
        if user_trash.exists() {
            let size = dir_size(&user_trash);
            if size > 0 {
                result.add("~/.Trash", user_trash, size);
            }
        }

        let uid = unsafe { libc::getuid() };
        let volumes = PathBuf::from("/Volumes");
        if volumes.exists() {
            if let Ok(entries) = std::fs::read_dir(&volumes) {
                for entry in entries.flatten() {
                    let vol_trash = entry.path().join(".Trashes").join(uid.to_string());
                    if vol_trash.exists() {
                        let size = dir_size(&vol_trash);
                        if size > 0 {
                            let label = format!("/Volumes/{}/.Trashes", entry.file_name().to_string_lossy());
                            result.add(label, vol_trash, size);
                        }
                    }
                }
            }
        }

        Ok(result)
    }

    fn clean(&self, result: &AnalysisResult, dry_run: bool, yes: bool) -> Result<()> {
        if result.items.is_empty() {
            println!("Trash is already empty.");
            return Ok(());
        }
        ui::print_analysis("Trash", &result.items);
        if dry_run { return Ok(()); }
        if !yes && !ui::confirm("Empty all trash?", false)? { return Ok(()); }
        for item in &result.items {
            match remove_dir_contents(&item.path) {
                Ok(_) => ui::print_ok(&format!("Cleared {}", item.label)),
                Err(e) => ui::print_warn(&format!("{}: {}", item.label, e)),
            }
        }
        Ok(())
    }
}
