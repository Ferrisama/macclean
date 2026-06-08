use std::path::PathBuf;
use anyhow::Result;
use crate::core::{AnalysisResult, Cleaner, CleanItem};
use crate::core::fs::dir_size;
use crate::core::cmd::{run_as_user, run_cmd};
use crate::ui;

pub struct BrewCleaner;

impl Cleaner for BrewCleaner {
    fn name(&self) -> &str { "brew" }
    fn display_name(&self) -> &str { "Homebrew" }

    fn analyze(&self) -> Result<AnalysisResult> {
        let mut result = AnalysisResult::default();
        if run_cmd(&["which", "brew"]).code != 0 { return Ok(result); }

        let cache_r = run_as_user(&["brew", "--cache"]);
        if cache_r.success() {
            let p = PathBuf::from(cache_r.output.trim());
            if p.exists() {
                let size = dir_size(&p);
                result.add("Homebrew download cache", p, size);
            }
        }

        let outdated_r = run_as_user(&["brew", "outdated", "--quiet"]);
        let count = outdated_r.output.lines().filter(|l| !l.trim().is_empty()).count();
        if count > 0 {
            result.items.push(CleanItem {
                label: format!("Outdated formulae ({} packages)", count),
                path: PathBuf::from("/dev/null"),
                size_bytes: 0,
                removable: false,
            });
        }
        Ok(result)
    }

    fn clean(&self, result: &AnalysisResult, dry_run: bool, yes: bool) -> Result<()> {
        if run_cmd(&["which", "brew"]).code != 0 {
            ui::print_warn("brew not found -- skipping.");
            return Ok(());
        }
        ui::print_analysis("Homebrew", &result.items);
        if dry_run { return Ok(()); }
        if !yes && !ui::confirm("Run brew cleanup and autoremove?", false)? { return Ok(()); }

        for (args, label) in &[
            (vec!["brew", "cleanup", "--prune=all"], "brew cleanup"),
            (vec!["brew", "autoremove"], "brew autoremove"),
        ] {
            let r = run_as_user(args);
            if r.success() { ui::print_ok(label); }
            else { ui::print_warn(&format!("{}: {}", label, &r.output[..r.output.len().min(200)])); }
        }
        Ok(())
    }
}
