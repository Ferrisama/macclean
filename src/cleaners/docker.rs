use crate::core::cmd::run_cmd;
use crate::core::{AnalysisResult, CleanItem, CleanKind, Cleaner, RiskLevel};
use crate::ui;
use anyhow::Result;
use std::path::PathBuf;

pub struct DockerCleaner;

impl Cleaner for DockerCleaner {
    fn name(&self) -> &str {
        "docker"
    }
    fn display_name(&self) -> &str {
        "Docker"
    }

    fn analyze(&self) -> Result<AnalysisResult> {
        let mut result = AnalysisResult::default();
        if run_cmd(&["which", "docker"]).code != 0 {
            return Ok(result);
        }
        if !run_cmd(&["docker", "info"]).success() {
            return Ok(result);
        }

        let df = run_cmd(&["docker", "system", "df"]);
        if df.success() {
            for line in df.output.lines().skip(1) {
                if let Some((label, size)) = parse_reclaimable_line(line) {
                    if size > 0 {
                        result.items.push(CleanItem {
                            label,
                            path: PathBuf::from("/dev/null"),
                            size_bytes: size,
                            removable: true,
                            kind: CleanKind::Cache,
                            risk: RiskLevel::Medium,
                            reason: "Docker reports this space as reclaimable.".into(),
                        });
                    }
                }
            }
        } else {
            result.items.push(CleanItem {
                label: "Docker reclaimable resources".into(),
                path: PathBuf::from("/dev/null"),
                size_bytes: 0,
                removable: true,
                kind: CleanKind::Cache,
                risk: RiskLevel::Medium,
                reason: "Docker prune removes stopped/unused resources, including volumes.".into(),
            });
        }

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
        if dry_run {
            return Ok(());
        }
        if !yes
            && !ui::confirm(
                "Prune Docker stopped containers, unused images, unused volumes, and build cache?",
                false,
            )?
        {
            return Ok(());
        }

        for (args, label) in &[
            (vec!["docker", "container", "prune", "-f"], "containers"),
            (vec!["docker", "image", "prune", "-af"], "images"),
            (vec!["docker", "volume", "prune", "-f"], "volumes"),
            (vec!["docker", "builder", "prune", "-af"], "build cache"),
        ] {
            let r = run_cmd(args);
            if r.success() {
                ui::print_ok(&format!("Pruned {}", label));
            } else {
                ui::print_warn(&format!(
                    "{}: {}",
                    label,
                    &r.output[..r.output.len().min(200)]
                ));
            }
        }
        Ok(())
    }
}

fn parse_reclaimable_line(line: &str) -> Option<(String, u64)> {
    let columns: Vec<&str> = line.split_whitespace().collect();
    if columns.is_empty() {
        return None;
    }

    let (label, reclaimable) = if line.starts_with("Local Volumes") {
        ("Unused Docker volumes", *columns.get(5)?)
    } else if line.starts_with("Build Cache") {
        ("Docker build cache", *columns.get(5)?)
    } else {
        let kind = *columns.first()?;
        let label = match kind {
            "Images" => "Unused Docker images",
            "Containers" => "Stopped Docker containers",
            _ => return None,
        };
        (label, *columns.get(4)?)
    };

    Some((label.into(), parse_size(reclaimable)?))
}

fn parse_size(raw: &str) -> Option<u64> {
    let (number, unit): (String, String) =
        raw.chars().partition(|c| c.is_ascii_digit() || *c == '.');
    let value: f64 = number.parse().ok()?;
    let multiplier = match unit.to_ascii_uppercase().as_str() {
        "B" => 1.0,
        "KB" => 1_000.0,
        "MB" => 1_000_000.0,
        "GB" => 1_000_000_000.0,
        "TB" => 1_000_000_000_000.0,
        _ => return None,
    };
    Some((value * multiplier) as u64)
}
