use std::path::PathBuf;
use anyhow::Result;
use crate::core::{AnalysisResult, Cleaner, CleanItem};
use crate::core::cmd::{run_cmd, require_sudo};
use crate::ui;

pub struct TimemachineCleaner;

impl Cleaner for TimemachineCleaner {
    fn name(&self) -> &str { "timemachine" }
    fn display_name(&self) -> &str { "Time Machine" }

    fn analyze(&self) -> Result<AnalysisResult> {
        let mut result = AnalysisResult::default();
        let r = run_cmd(&["tmutil", "listlocalsnapshots", "/"]);
        if !r.success() { return Ok(result); }

        for line in r.output.lines() {
            let line = line.trim();
            if line.starts_with("com.apple.TimeMachine") {
                result.items.push(CleanItem {
                    label: line.to_string(),
                    path: PathBuf::from("/"),
                    size_bytes: 0,
                    removable: true,
                });
            }
        }

        Ok(result)
    }

    fn clean(&self, result: &AnalysisResult, dry_run: bool, yes: bool) -> Result<()> {
        if result.items.is_empty() {
            println!("No local Time Machine snapshots found.");
            return Ok(());
        }
        ui::print_analysis("Time Machine Local Snapshots", &result.items);
        if dry_run { return Ok(()); }
        if !yes && !ui::confirm(&format!("Delete all {} local snapshot(s)?", result.items.len()), false)? {
            return Ok(());
        }
        require_sudo();

        for item in &result.items {
            // Parse date part: e.g. "com.apple.TimeMachine.2024-01-15-120000.local"
            // Find the component starting with a digit
            let date_part = item.label
                .split('.')
                .find(|s| s.starts_with(|c: char| c.is_ascii_digit()))
                .unwrap_or(item.label.as_str());

            let r = run_cmd(&["tmutil", "deletelocalsnapshots", date_part]);
            if r.success() {
                ui::print_ok(&format!("Deleted snapshot {}", date_part));
            } else {
                ui::print_warn(&format!("{}: {}", item.label, &r.output[..r.output.len().min(200)]));
            }
        }
        Ok(())
    }
}
