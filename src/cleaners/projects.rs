use std::collections::HashMap;
use std::path::PathBuf;
use anyhow::Result;
use walkdir::WalkDir;
use crate::core::{AnalysisResult, Cleaner};
use crate::core::fs::dir_size;
use crate::ui;

pub struct ProjectsCleaner;

const SKIP_DIRS: &[&str] = &[".git", ".ssh", "Library", "Applications", ".Trash"];

impl Cleaner for ProjectsCleaner {
    fn name(&self) -> &str { "projects" }
    fn display_name(&self) -> &str { "Project Artifacts" }

    fn analyze(&self) -> Result<AnalysisResult> {
        let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("/tmp"));
        let mut result = AnalysisResult::default();

        let artifact_names: HashMap<&str, &str> = [
            ("node_modules", "Node.js dependencies"),
            (".venv", "Python virtualenv"),
            ("venv", "Python virtualenv"),
            ("env", "Python virtualenv"),
            ("__pycache__", "Python cache"),
            ("build", "Build output"),
            ("dist", "Distribution output"),
            ("target", "Rust/Java build output"),
            (".next", "Next.js build cache"),
            (".nuxt", "Nuxt.js build cache"),
            (".parcel-cache", "Parcel cache"),
            (".turbo", "Turborepo cache"),
        ].into_iter().collect();

        for entry in WalkDir::new(&home)
            .max_depth(5)
            .follow_links(false)
            .into_iter()
            .filter_map(|e| e.ok())
        {
            if !entry.file_type().is_dir() { continue; }

            let name = entry.file_name().to_string_lossy();

            // Skip dirs we should never descend into or report
            if SKIP_DIRS.iter().any(|s| *s == name.as_ref()) { continue; }

            if let Some(&artifact_type) = artifact_names.get(name.as_ref()) {
                let path = entry.into_path();
                let size = dir_size(&path);
                if size > 0 {
                    let rel = path
                        .strip_prefix(&home)
                        .map(|p| format!("~/{}", p.display()))
                        .unwrap_or_else(|_| path.display().to_string());
                    result.add(format!("{} -- {}", artifact_type, rel), path, size);
                }
            }
        }

        Ok(result)
    }

    fn clean(&self, result: &AnalysisResult, dry_run: bool, yes: bool) -> Result<()> {
        if result.items.is_empty() {
            println!("No project artifact directories found.");
            return Ok(());
        }

        // Show up to 40 results
        let display_items: Vec<_> = result.items.iter().take(40).cloned().collect();
        ui::print_analysis("Project Artifacts", &display_items);
        if result.items.len() > 40 {
            ui::print_warn(&format!(
                "... and {} more (showing 40 of {})",
                result.items.len() - 40,
                result.items.len()
            ));
        }

        if dry_run { return Ok(()); }
        if !yes && !ui::confirm(
            &format!("Delete all {} artifact directories?", result.items.len()),
            false,
        )? {
            return Ok(());
        }

        for item in &result.items {
            match std::fs::remove_dir_all(&item.path) {
                Ok(_) => ui::print_ok(&format!("Removed {}", item.label)),
                Err(e) => ui::print_warn(&format!("{}: {}", item.label, e)),
            }
        }
        Ok(())
    }
}
