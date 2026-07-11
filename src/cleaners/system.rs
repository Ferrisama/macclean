use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};
use anyhow::Result;
use walkdir::WalkDir;
use crate::core::{AnalysisResult, Cleaner};
use crate::core::fs::dir_size;
use crate::core::cmd::{run_cmd, require_sudo};
use crate::ui;

pub struct SystemCleaner;

/// /private/tmp is shared, machine-wide scratch space -- other users' sessions
/// and other processes' sockets/lockfiles can live there. Only entries this
/// old are safe to assume abandoned; this matches the age macOS's own
/// `periodic daily` temp-file reaping uses.
const TMP_MIN_AGE: Duration = Duration::from_secs(24 * 60 * 60);

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

        let cache_dir = PathBuf::from("/Library/Caches");
        if cache_dir.exists() {
            let size = dir_size(&cache_dir);
            if size > 0 {
                result.add("/Library/Caches", cache_dir, size);
            }
        }

        let log_dir = PathBuf::from("/private/var/log");
        let rotated_size = scoped_size(&log_dir, is_rotated_log);
        if rotated_size > 0 {
            result.add("/private/var/log (rotated logs only)", log_dir, rotated_size);
        }

        let tmp_dir = PathBuf::from("/private/tmp");
        let stale_size = scoped_size(&tmp_dir, |p| is_stale(p, TMP_MIN_AGE));
        if stale_size > 0 {
            result.add("/private/tmp (files >24h old)", tmp_dir, stale_size);
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
            let outcome = if item.path == Path::new("/private/var/log") {
                remove_scoped(&item.path, is_rotated_log)
            } else if item.path == Path::new("/private/tmp") {
                remove_scoped(&item.path, |p| is_stale(p, TMP_MIN_AGE))
            } else {
                crate::core::fs::remove_dir_contents(&item.path)
            };

            match outcome {
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

/// Sum bytes of files anywhere under `dir` whose path passes `keep`.
fn scoped_size(dir: &Path, keep: impl Fn(&Path) -> bool) -> u64 {
    WalkDir::new(dir)
        .follow_links(false)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
        .filter(|e| keep(e.path()))
        .filter_map(|e| e.metadata().ok())
        .map(|m| m.len())
        .sum()
}

/// Remove only files anywhere under `dir` whose path passes `keep`, leaving
/// everything else -- including the directory structure itself -- untouched.
fn remove_scoped(dir: &Path, keep: impl Fn(&Path) -> bool) -> Result<()> {
    if !dir.exists() {
        return Ok(());
    }
    for entry in WalkDir::new(dir)
        .follow_links(false)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
        .filter(|e| keep(e.path()))
    {
        std::fs::remove_file(entry.path()).ok();
    }
    Ok(())
}

/// Already-rotated/archived log files (e.g. `system.log.1`, `install.log.0.gz`) --
/// never a live log a daemon is currently appending to.
fn is_rotated_log(path: &Path) -> bool {
    let Some(name) = path.file_name().and_then(|n| n.to_str()) else { return false; };
    if name.ends_with(".gz") || name.ends_with(".bz2") || name.ends_with(".Z") {
        return true;
    }
    name.rsplit('.')
        .next()
        .is_some_and(|last| !last.is_empty() && last.chars().all(|c| c.is_ascii_digit()))
}

fn is_stale(path: &Path, min_age: Duration) -> bool {
    let Ok(meta) = path.symlink_metadata() else { return false; };
    let Ok(modified) = meta.modified() else { return false; };
    SystemTime::now()
        .duration_since(modified)
        .is_ok_and(|age| age >= min_age)
}
