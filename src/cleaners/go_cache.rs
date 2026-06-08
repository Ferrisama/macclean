use std::path::PathBuf;
use anyhow::Result;
use crate::core::{AnalysisResult, Cleaner};
use crate::core::fs::{dir_size, remove_dir_contents};
use crate::core::cmd::run_cmd;
use crate::ui;

pub struct GoCacheCleaner;

impl Cleaner for GoCacheCleaner {
    fn name(&self) -> &str { "go" }
    fn display_name(&self) -> &str { "Go Module Cache" }

    fn analyze(&self) -> Result<AnalysisResult> {
        let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("/tmp"));
        let mut result = AnalysisResult::default();

        let path = home.join("go/pkg");
        if path.exists() {
            let size = dir_size(&path);
            if size > 0 {
                result.add("Go module cache (~/go/pkg)", path, size);
            }
        }

        Ok(result)
    }

    fn clean(&self, result: &AnalysisResult, dry_run: bool, yes: bool) -> Result<()> {
        if result.items.is_empty() {
            println!("No Go module cache found.");
            return Ok(());
        }
        ui::print_analysis("Go Module Cache", &result.items);
        if dry_run { return Ok(()); }
        if !yes && !ui::confirm("Clear Go module cache?", false)? { return Ok(()); }
        for item in &result.items {
            match remove_dir_contents(&item.path) {
                Ok(_) => ui::print_ok(&format!("Cleared {}", item.label)),
                Err(e) => ui::print_warn(&format!("{}: {}", item.label, e)),
            }
        }
        let res = run_cmd(&["go", "clean", "-modcache"]);
        if res.success() {
            ui::print_ok("go clean -modcache completed");
        } else {
            ui::print_warn(&format!("go clean -modcache: {}", res.output));
        }
        Ok(())
    }
}
