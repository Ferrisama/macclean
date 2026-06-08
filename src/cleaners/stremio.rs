use std::path::PathBuf;
use anyhow::Result;
use crate::core::{AnalysisResult, Cleaner};
use crate::core::fs::{dir_size, remove_dir_contents};
use crate::ui;

pub struct StremioCleaner;

impl Cleaner for StremioCleaner {
    fn name(&self) -> &str { "stremio" }
    fn display_name(&self) -> &str { "Stremio Cache" }

    fn analyze(&self) -> Result<AnalysisResult> {
        let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("/tmp"));
        let mut result = AnalysisResult::default();

        let path = home.join("Library/Application Support/stremio-server/stremio-cache");
        if path.exists() {
            let size = dir_size(&path);
            if size > 0 {
                result.add("stremio-server video cache", path, size);
            }
        }

        Ok(result)
    }

    fn clean(&self, result: &AnalysisResult, dry_run: bool, yes: bool) -> Result<()> {
        if result.items.is_empty() {
            println!("No Stremio cache found.");
            return Ok(());
        }
        ui::print_analysis("Stremio Cache", &result.items);
        if dry_run { return Ok(()); }
        if !yes && !ui::confirm("Clear Stremio video cache?", false)? { return Ok(()); }
        for item in &result.items {
            match remove_dir_contents(&item.path) {
                Ok(_) => ui::print_ok(&format!("Cleared {}", item.label)),
                Err(e) => ui::print_warn(&format!("{}: {}", item.label, e)),
            }
        }
        Ok(())
    }
}
