use std::path::PathBuf;
use anyhow::Result;
use crate::core::{AnalysisResult, Cleaner, CleanItem};
use crate::core::cmd::run_cmd;
use crate::ui;

pub struct DockerCleaner;

impl Cleaner for DockerCleaner {
    fn name(&self) -> &str { "docker" }
    fn display_name(&self) -> &str { "Docker" }

    fn analyze(&self) -> Result<AnalysisResult> {
        let mut result = AnalysisResult::default();
        if run_cmd(&["which", "docker"]).code != 0 { return Ok(result); }
        if !run_cmd(&["docker", "info"]).success() { return Ok(result); }

        for (args, label) in &[
            (vec!["docker", "image", "ls", "-q"], "Unused images"),
            (vec!["docker", "ps", "-aq", "-f", "status=exited"], "Stopped containers"),
            (vec!["docker", "volume", "ls", "-q"], "Unused volumes"),
        ] {
            let r = run_cmd(args);
            let count = r.output.lines().filter(|l| !l.trim().is_empty()).count();
            if count > 0 {
                result.items.push(CleanItem {
                    label: format!("{} ({})", label, count),
                    path: PathBuf::from("/dev/null"),
                    size_bytes: 0,
                    removable: true,
                });
            }
        }
        result.items.push(CleanItem {
            label: "Build cache + reclaimable".into(),
            path: PathBuf::from("/dev/null"),
            size_bytes: 0,
            removable: true,
        });
        Ok(result)
    }

    fn clean(&self, result: &AnalysisResult, dry_run: bool, yes: bool) -> Result<()> {
        if run_cmd(&["which", "docker"]).code != 0 {
            ui::print_warn("docker not found -- skipping.");
            return Ok(());
        }
        if !run_cmd(&["docker", "info"]).success() {
            ui::print_warn("Docker daemon not running -- skipping.");
            return Ok(());
        }
        ui::print_analysis("Docker", &result.items);
        if dry_run { return Ok(()); }
        if !yes && !ui::confirm("Prune all Docker resources?", false)? { return Ok(()); }

        for (args, label) in &[
            (vec!["docker", "container", "prune", "-f"], "containers"),
            (vec!["docker", "image", "prune", "-af"], "images"),
            (vec!["docker", "volume", "prune", "-f"], "volumes"),
            (vec!["docker", "builder", "prune", "-af"], "build cache"),
        ] {
            let r = run_cmd(args);
            if r.success() { ui::print_ok(&format!("Pruned {}", label)); }
            else { ui::print_warn(&format!("{}: {}", label, &r.output[..r.output.len().min(200)])); }
        }
        Ok(())
    }
}
