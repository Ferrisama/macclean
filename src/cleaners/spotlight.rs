use anyhow::Result;
use crate::core::{AnalysisResult, Cleaner};
use crate::core::cmd::{run_cmd, require_sudo};
use crate::ui;

pub struct SpotlightCleaner;

impl Cleaner for SpotlightCleaner {
    fn name(&self) -> &str { "spotlight" }
    fn display_name(&self) -> &str { "Spotlight" }

    fn analyze(&self) -> Result<AnalysisResult> {
        Ok(AnalysisResult::default())
    }

    fn clean(&self, _result: &AnalysisResult, dry_run: bool, yes: bool) -> Result<()> {
        println!("  This will reindex the entire drive (Spotlight unavailable for ~10 min).");
        if dry_run { return Ok(()); }
        if !yes && !ui::confirm("Reindex Spotlight?", false)? { return Ok(()); }

        require_sudo();
        let r = run_cmd(&["mdutil", "-E", "/"]);
        if r.success() {
            ui::print_ok("Spotlight reindex started.");
        } else {
            ui::print_warn(&format!("mdutil failed: {}", &r.output[..r.output.len().min(200)]));
        }
        Ok(())
    }
}
