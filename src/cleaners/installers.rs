use std::path::PathBuf;
use anyhow::Result;
use crate::core::{AnalysisResult, Cleaner};
use crate::ui;

const MIN_BYTES: u64 = 5 * 1024 * 1024;

pub struct InstallersCleaner;

impl Cleaner for InstallersCleaner {
    fn name(&self) -> &str { "installers" }
    fn display_name(&self) -> &str { "Installer Files" }

    fn analyze(&self) -> Result<AnalysisResult> {
        let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("/tmp"));
        let mut result = AnalysisResult::default();

        for dir_name in &["Downloads", "Desktop"] {
            let scan_dir = home.join(dir_name);
            if !scan_dir.exists() { continue; }

            let Ok(entries) = std::fs::read_dir(&scan_dir) else { continue; };
            for entry in entries.flatten() {
                let path = entry.path();
                if !path.is_file() { continue; }

                let name = path.file_name().unwrap_or_default().to_string_lossy().to_lowercase();
                let is_installer = matches!(path.extension().and_then(|e| e.to_str()),
                    Some("dmg") | Some("pkg") | Some("zip"))
                    || name.ends_with(".tar.gz") || name.ends_with(".tar.bz2");

                if !is_installer { continue; }

                let Ok(meta) = path.metadata() else { continue; };
                let size = meta.len();
                if size < MIN_BYTES { continue; }

                let age_days = meta.modified()
                    .ok()
                    .and_then(|t| t.elapsed().ok())
                    .map(|d| d.as_secs() / 86400)
                    .unwrap_or(0);

                let label = format!("{} ({}, {}d old)",
                    path.file_name().unwrap_or_default().to_string_lossy(),
                    dir_name, age_days);
                result.add(label, path, size);
            }
        }
        Ok(result)
    }

    fn clean(&self, result: &AnalysisResult, dry_run: bool, yes: bool) -> Result<()> {
        if result.items.is_empty() {
            println!("No installer files found.");
            return Ok(());
        }
        ui::print_analysis("Installer Files", &result.items);
        if dry_run { return Ok(()); }
        if !yes && !ui::confirm("Delete all listed installer files?", false)? { return Ok(()); }
        for item in &result.items {
            match std::fs::remove_file(&item.path) {
                Ok(_) => ui::print_ok(&format!("Deleted {}", item.label)),
                Err(e) => ui::print_warn(&format!("{}: {}", item.label, e)),
            }
        }
        Ok(())
    }
}
