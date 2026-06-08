use std::path::PathBuf;
use anyhow::Result;
use crate::core::{AnalysisResult, Cleaner};
use crate::core::fs::{dir_size, remove_dir_contents};
use crate::core::cmd::{run_cmd, require_sudo};
use crate::ui;

pub struct SystemCleaner;

impl Cleaner for SystemCleaner {
    fn name(&self) -> &str { "system" }
    fn display_name(&self) -> &str { "System Caches & Logs" }

    fn analyze(&self) -> Result<AnalysisResult> {
        let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("/tmp"));
        let mut result = AnalysisResult::default();

        let home_dirs = [
            ("User Caches", "Library/Caches"),
            ("User Logs", "Library/Logs"),
        ];
        for (label, rel) in &home_dirs {
            let path = home.join(rel);
            if path.exists() {
                let size = dir_size(&path);
                if size > 0 {
                    result.add(*label, path, size);
                }
            }
        }

        let abs_dirs: &[&str] = &[
            "/Library/Caches",
            "/private/var/log",
            "/private/tmp",
        ];
        for abs in abs_dirs {
            let path = PathBuf::from(abs);
            if path.exists() {
                let size = dir_size(&path);
                if size > 0 {
                    result.add(*abs, path, size);
                }
            }
        }

        Ok(result)
    }

    fn clean(&self, result: &AnalysisResult, dry_run: bool, yes: bool) -> Result<()> {
        if result.items.is_empty() {
            println!("No system caches or logs found.");
            return Ok(());
        }
        ui::print_analysis("System Caches & Logs", &result.items);
        if dry_run { return Ok(()); }
        if !yes && !ui::confirm("Clean system caches and logs?", false)? { return Ok(()); }

        require_sudo();

        for item in &result.items {
            match remove_dir_contents(&item.path) {
                Ok(_) => ui::print_ok(&format!("Cleared {}", item.label)),
                Err(e) => ui::print_warn(&format!("{}: {}", item.label, e)),
            }
        }

        run_cmd(&["/usr/sbin/periodic", "daily", "weekly", "monthly"]);
        run_cmd(&["dscacheutil", "-flushcache"]);
        run_cmd(&["killall", "-HUP", "mDNSResponder"]);
        ui::print_ok("DNS cache flushed");

        Ok(())
    }
}
