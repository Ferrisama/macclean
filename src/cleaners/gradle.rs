use std::path::PathBuf;
use anyhow::Result;
use crate::core::{AnalysisResult, Cleaner};
use crate::core::fs::{dir_size, remove_dir_contents};
use crate::ui;

pub struct GradleCleaner;

impl Cleaner for GradleCleaner {
    fn name(&self) -> &str { "gradle" }
    fn display_name(&self) -> &str { "Gradle Cache" }

    fn analyze(&self) -> Result<AnalysisResult> {
        let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("/tmp"));
        let mut result = AnalysisResult::default();

        let path = home.join(".gradle/caches");
        if path.exists() {
            let size = dir_size(&path);
            if size > 0 {
                result.add("Gradle caches", path, size);
            }
        }

        Ok(result)
    }

    fn clean(&self, result: &AnalysisResult, dry_run: bool, yes: bool) -> Result<()> {
        if result.items.is_empty() {
            println!("No Gradle caches found.");
            return Ok(());
        }
        ui::print_analysis("Gradle Cache", &result.items);
        if dry_run { return Ok(()); }
        if !yes && !ui::confirm("Clear Gradle caches?", false)? { return Ok(()); }
        for item in &result.items {
            match remove_dir_contents(&item.path) {
                Ok(_) => ui::print_ok(&format!("Cleared {}", item.label)),
                Err(e) => ui::print_warn(&format!("{}: {}", item.label, e)),
            }
        }
        Ok(())
    }
}
