use crate::core::fs::{dir_size, remove_dir_contents};
use crate::core::{AnalysisResult, Cleaner};
use crate::ui;
use anyhow::Result;
use std::path::PathBuf;

pub struct StremioCleaner;

impl Cleaner for StremioCleaner {
    fn name(&self) -> &str {
        "stremio"
    }
    fn display_name(&self) -> &str {
        "Stremio Caches"
    }

    fn analyze(&self) -> Result<AnalysisResult> {
        let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("/tmp"));
        let mut result = AnalysisResult::default();

        let cache_dirs = [
            (
                "stremio-server video cache",
                "Library/Application Support/stremio-server/stremio-cache",
            ),
            (
                "stremio-server cache",
                "Library/Application Support/stremio-server/server-cache",
            ),
            (
                "Stremio desktop cache",
                "Library/Application Support/Smart Code ltd/Stremio/Cache",
            ),
            (
                "Stremio Chromium code cache",
                "Library/Application Support/Smart Code ltd/Stremio/Code Cache",
            ),
            (
                "Stremio GPU cache",
                "Library/Application Support/Smart Code ltd/Stremio/GPUCache",
            ),
            (
                "Stremio Dawn cache",
                "Library/Application Support/Smart Code ltd/Stremio/DawnCache",
            ),
            (
                "Stremio service worker cache",
                "Library/Application Support/Smart Code ltd/Stremio/Service Worker/CacheStorage",
            ),
            (
                "Stremio blob cache",
                "Library/Application Support/Smart Code ltd/Stremio/blob_storage",
            ),
            (
                "Stremio WebKit data",
                "Library/WebKit/com.westbridge.stremio5-mac",
            ),
            (
                "Stremio shell WebKit data",
                "Library/WebKit/com.stremio.stremio-shell-macos",
            ),
        ];

        for (label, rel) in cache_dirs {
            let path = home.join(rel);
            if path.exists() {
                let size = dir_size(&path);
                if size > 0 {
                    result.add(label, path, size);
                }
            }
        }

        Ok(result)
    }

    fn clean(&self, result: &AnalysisResult, dry_run: bool, yes: bool) -> Result<()> {
        if result.items.is_empty() {
            println!("No Stremio caches found.");
            return Ok(());
        }
        ui::print_analysis("Stremio Caches", &result.items);
        if dry_run {
            return Ok(());
        }
        if !yes
            && !ui::confirm(
                "Clear Stremio caches and local WebKit data? This may sign Stremio out.",
                false,
            )?
        {
            return Ok(());
        }
        for item in &result.items {
            match remove_dir_contents(&item.path) {
                Ok(_) => ui::print_ok(&format!("Cleared {}", item.label)),
                Err(e) => ui::print_warn(&format!("{}: {}", item.label, e)),
            }
        }
        Ok(())
    }
}
