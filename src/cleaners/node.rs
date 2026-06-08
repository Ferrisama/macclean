use std::path::PathBuf;
use anyhow::Result;
use crate::core::{AnalysisResult, Cleaner};
use crate::core::fs::{dir_size, remove_dir_contents};
use crate::ui;

pub struct NodeCleaner;

impl Cleaner for NodeCleaner {
    fn name(&self) -> &str { "node" }
    fn display_name(&self) -> &str { "Node.js Caches" }

    fn analyze(&self) -> Result<AnalysisResult> {
        let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("/tmp"));
        let mut result = AnalysisResult::default();

        let node_dirs = [
            ("npm cache", ".npm"),
            ("yarn cache", ".yarn/cache"),
            ("pnpm store", ".local/share/pnpm/store"),
            ("pnpm cache", "Library/Caches/pnpm"),
        ];

        for (label, rel) in &node_dirs {
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
            println!("No Node.js caches found.");
            return Ok(());
        }
        ui::print_analysis("Node.js Caches", &result.items);
        if dry_run { return Ok(()); }
        if !yes && !ui::confirm("Clear Node.js caches?", false)? { return Ok(()); }
        for item in &result.items {
            match remove_dir_contents(&item.path) {
                Ok(_) => ui::print_ok(&format!("Cleared {}", item.label)),
                Err(e) => ui::print_warn(&format!("{}: {}", item.label, e)),
            }
        }
        Ok(())
    }
}
