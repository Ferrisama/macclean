use anyhow::Result;
use crate::core::{AnalysisResult, Cleaner};
use crate::core::cmd::{run_cmd, require_sudo};
use crate::ui;

pub struct MemoryCleaner;

impl Cleaner for MemoryCleaner {
    fn name(&self) -> &str { "memory" }
    fn display_name(&self) -> &str { "Memory" }

    fn analyze(&self) -> Result<AnalysisResult> {
        Ok(AnalysisResult::default())
    }

    fn clean(&self, _result: &AnalysisResult, dry_run: bool, yes: bool) -> Result<()> {
        // Get total RAM
        let memsize_r = run_cmd(&["sysctl", "-n", "hw.memsize"]);
        let total_bytes: u64 = memsize_r.output.trim().parse().unwrap_or(0);

        // Get page size
        let pagesize_r = run_cmd(&["sysctl", "-n", "vm.pagesize"]);
        let page_size: u64 = pagesize_r.output.trim().parse().unwrap_or(4096);

        // Parse vm_stat for inactive pages
        let vm_r = run_cmd(&["vm_stat"]);
        let mut inactive_pages: u64 = 0;
        for line in vm_r.output.lines() {
            if line.trim_start().starts_with("Pages inactive:") {
                let num_str = line
                    .split(':')
                    .nth(1)
                    .unwrap_or("0")
                    .trim()
                    .trim_end_matches('.');
                inactive_pages = num_str.parse().unwrap_or(0);
                break;
            }
        }

        let inactive_bytes = inactive_pages * page_size;
        println!(
            "  RAM: {} total, {} inactive",
            ui::format_size(total_bytes),
            ui::format_size(inactive_bytes)
        );

        if dry_run { return Ok(()); }
        if !yes && !ui::confirm("Run sudo purge?", false)? { return Ok(()); }

        require_sudo();
        let r = run_cmd(&["purge"]);
        if r.success() {
            ui::print_ok("Inactive memory flushed.");
        } else {
            ui::print_warn(&format!("purge failed: {}", &r.output[..r.output.len().min(200)]));
        }
        Ok(())
    }
}
