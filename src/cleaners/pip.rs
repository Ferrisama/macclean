use std::path::PathBuf;
use anyhow::Result;
use crate::core::{AnalysisResult, Cleaner};
use crate::core::fs::{dir_size, remove_dir_contents};
use crate::core::cmd::run_as_user;
use crate::ui;

pub struct PipCleaner;

impl Cleaner for PipCleaner {
    fn name(&self) -> &str { "pip" }
    fn display_name(&self) -> &str { "pip Cache" }

    fn analyze(&self) -> Result<AnalysisResult> {
        let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("/tmp"));
        let mut result = AnalysisResult::default();

        let pip_dirs = [
            ("pip cache (user)", "Library/Caches/pip"),
            ("pip cache (XDG)", ".cache/pip"),
        ];

        for (label, rel) in &pip_dirs {
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
            println!("No pip caches found.");
            return Ok(());
        }
        ui::print_analysis("pip Cache", &result.items);
        if dry_run { return Ok(()); }
        if !yes && !ui::confirm("Clear pip caches?", false)? { return Ok(()); }
        for item in &result.items {
            match remove_dir_contents(&item.path) {
                Ok(_) => ui::print_ok(&format!("Cleared {}", item.label)),
                Err(e) => ui::print_warn(&format!("{}: {}", item.label, e)),
            }
        }
        // Also purge via pip command; ignore errors
        run_as_user(&["pip", "cache", "purge"]);
        Ok(())
    }
}
