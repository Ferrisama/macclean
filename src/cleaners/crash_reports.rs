use std::path::PathBuf;
use anyhow::Result;
use crate::core::{AnalysisResult, Cleaner};
use crate::core::fs::{dir_size, remove_dir_contents};
use crate::core::cmd::require_sudo;
use crate::ui;

pub struct CrashReportsCleaner;

impl Cleaner for CrashReportsCleaner {
    fn name(&self) -> &str { "crash-reports" }
    fn display_name(&self) -> &str { "Crash Reports" }

    fn analyze(&self) -> Result<AnalysisResult> {
        let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("/tmp"));
        let mut result = AnalysisResult::default();

        let user_reports = home.join("Library/Logs/DiagnosticReports");
        if user_reports.exists() {
            let size = dir_size(&user_reports);
            if size > 0 {
                result.add("~/Library/Logs/DiagnosticReports", user_reports, size);
            }
        }

        let system_reports = PathBuf::from("/Library/Logs/DiagnosticReports");
        if system_reports.exists() {
            let size = dir_size(&system_reports);
            if size > 0 {
                result.add("/Library/Logs/DiagnosticReports", system_reports, size);
            }
        }

        Ok(result)
    }

    fn clean(&self, result: &AnalysisResult, dry_run: bool, yes: bool) -> Result<()> {
        if result.items.is_empty() {
            println!("No crash reports found.");
            return Ok(());
        }
        ui::print_analysis("Crash Reports", &result.items);
        if dry_run { return Ok(()); }
        if !yes && !ui::confirm("Remove all crash reports?", false)? { return Ok(()); }

        require_sudo();

        for item in &result.items {
            match remove_dir_contents(&item.path) {
                Ok(_) => {
                    ui::print_ok(&format!("Cleared {}", item.label));
                    std::fs::create_dir_all(&item.path).ok();
                }
                Err(e) => ui::print_warn(&format!("{}: {}", item.label, e)),
            }
        }
        Ok(())
    }
}
