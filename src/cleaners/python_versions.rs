use std::path::PathBuf;
use anyhow::Result;
use crate::core::{AnalysisResult, Cleaner, CleanItem};
use crate::core::fs::dir_size;
use crate::core::cmd::run_cmd;
use crate::ui;

pub struct PythonVersionsCleaner;

impl Cleaner for PythonVersionsCleaner {
    fn name(&self) -> &str { "python" }
    fn display_name(&self) -> &str { "Python Versions (pyenv)" }

    fn analyze(&self) -> Result<AnalysisResult> {
        let mut result = AnalysisResult::default();

        if !run_cmd(&["which", "pyenv"]).success() {
            return Ok(result);
        }

        // Get pyenv root
        let root_r = run_cmd(&["pyenv", "root"]);
        let pyenv_root = if root_r.success() {
            PathBuf::from(root_r.output.trim())
        } else {
            dirs::home_dir()
                .unwrap_or_else(|| PathBuf::from("/tmp"))
                .join(".pyenv")
        };

        let versions_dir = pyenv_root.join("versions");
        if !versions_dir.exists() {
            return Ok(result);
        }

        // Get active version
        let active_r = run_cmd(&["pyenv", "version-name"]);
        let active = active_r.output.trim().to_string();

        let entries = match std::fs::read_dir(&versions_dir) {
            Ok(e) => e,
            Err(_) => return Ok(result),
        };

        for entry in entries.filter_map(|e| e.ok()) {
            let path = entry.path();
            if !path.is_dir() { continue; }

            let version_name = path
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_default();

            let is_active = version_name == active;

            // Check if it has virtualenvs
            let envs_dir = path.join("envs");
            let has_envs = envs_dir.exists()
                && std::fs::read_dir(&envs_dir)
                    .map(|mut d| d.next().is_some())
                    .unwrap_or(false);

            let mut label = version_name.clone();
            if is_active { label.push_str(" [active]"); }
            if has_envs { label.push_str(" [has virtualenvs]"); }

            let removable = !is_active && !has_envs;
            let size = dir_size(&path);

            result.items.push(CleanItem {
                label,
                path,
                size_bytes: size,
                removable,
            });
        }

        Ok(result)
    }

    fn clean(&self, result: &AnalysisResult, dry_run: bool, yes: bool) -> Result<()> {
        if result.items.is_empty() {
            println!("No pyenv Python versions found.");
            return Ok(());
        }

        ui::print_analysis("Python Versions (pyenv)", &result.items);

        let removable: Vec<_> = result.items.iter().filter(|i| i.removable).collect();
        if removable.is_empty() {
            println!("No unused Python versions found.");
            return Ok(());
        }

        if dry_run { return Ok(()); }
        if !yes && !ui::confirm(
            &format!("Uninstall {} unused Python version(s)?", removable.len()),
            false,
        )? {
            return Ok(());
        }

        for item in &removable {
            // Extract the plain version name (strip any suffix tags we added)
            let version_name = item.label
                .split_whitespace()
                .next()
                .unwrap_or(&item.label);

            let r = run_cmd(&["pyenv", "uninstall", "-f", version_name]);
            if r.success() {
                ui::print_ok(&format!("Uninstalled Python {}", version_name));
            } else {
                ui::print_warn(&format!("{}: {}", version_name, &r.output[..r.output.len().min(200)]));
            }
        }
        Ok(())
    }
}
