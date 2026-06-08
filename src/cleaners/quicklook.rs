use std::path::PathBuf;
use anyhow::Result;
use crate::core::{AnalysisResult, Cleaner};
use crate::core::fs::dir_size;
use crate::core::cmd::run_cmd;
use crate::ui;

pub struct QuickLookCleaner;

impl Cleaner for QuickLookCleaner {
    fn name(&self) -> &str { "quicklook" }
    fn display_name(&self) -> &str { "QuickLook Cache" }

    fn analyze(&self) -> Result<AnalysisResult> {
        let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("/tmp"));
        let mut result = AnalysisResult::default();

        let ql_dirs = [
            ("com.apple.QuickLookDaemon", "Library/Caches/com.apple.QuickLookDaemon"),
            ("com.apple.quicklook.ThumbnailsAgent", "Library/Caches/com.apple.quicklook.ThumbnailsAgent"),
        ];

        for (label, rel) in &ql_dirs {
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
            println!("No QuickLook cache found.");
            return Ok(());
        }
        ui::print_analysis("QuickLook Cache", &result.items);
        if dry_run { return Ok(()); }
        if !yes && !ui::confirm("Kill and rebuild QuickLook server?", false)? { return Ok(()); }

        run_cmd(&["qlmanage", "-r"]);
        run_cmd(&["qlmanage", "-r", "cache"]);
        ui::print_ok("QuickLook server reset and cache rebuilt");

        Ok(())
    }
}
