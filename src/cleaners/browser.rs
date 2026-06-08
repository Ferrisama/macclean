use std::path::PathBuf;
use anyhow::Result;
use crate::core::{AnalysisResult, Cleaner};
use crate::core::fs::{dir_size, remove_dir_contents};
use crate::ui;

pub struct BrowserCleaner;

impl Cleaner for BrowserCleaner {
    fn name(&self) -> &str { "browser" }
    fn display_name(&self) -> &str { "Browser Caches" }

    fn analyze(&self) -> Result<AnalysisResult> {
        let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("/tmp"));
        let mut result = AnalysisResult::default();

        let browser_dirs = [
            ("Safari", "Library/Caches/com.apple.Safari"),
            ("Chrome", "Library/Caches/Google/Chrome"),
            ("Firefox", "Library/Caches/Firefox"),
            ("Edge", "Library/Caches/Microsoft Edge"),
            ("Brave", "Library/Caches/BraveSoftware"),
        ];

        for (label, rel) in &browser_dirs {
            let path = home.join(rel);
            if path.exists() {
                let size = dir_size(&path);
                if size > 0 {
                    result.add(*label, path, size);
                }
            }
        }

        Ok(result)
    }

    fn clean(&self, result: &AnalysisResult, dry_run: bool, yes: bool) -> Result<()> {
        if result.items.is_empty() {
            println!("No browser caches found.");
            return Ok(());
        }
        ui::print_analysis("Browser Caches", &result.items);
        if dry_run { return Ok(()); }
        if !yes && !ui::confirm("Clear browser caches?", false)? { return Ok(()); }
        for item in &result.items {
            match remove_dir_contents(&item.path) {
                Ok(_) => ui::print_ok(&format!("Cleared {}", item.label)),
                Err(e) => ui::print_warn(&format!("{}: {}", item.label, e)),
            }
        }
        Ok(())
    }
}
