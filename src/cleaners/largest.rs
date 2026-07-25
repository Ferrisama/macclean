use crate::core::{AnalysisResult, CleanKind, RiskLevel};
use crate::ui::{self, format_size};
use anyhow::Result;
use colored::Colorize;
use rayon::iter::ParallelBridge;
use rayon::prelude::*;
use std::path::PathBuf;
use walkdir::WalkDir;

pub fn run(
    min_mb: u64,
    limit: usize,
    scan_path: Option<PathBuf>,
    trash: bool,
    dry_run: bool,
    yes: bool,
) -> Result<()> {
    let root = scan_path.unwrap_or_else(|| dirs::home_dir().unwrap_or_else(|| PathBuf::from(".")));
    let min_bytes = min_mb * 1024 * 1024;

    println!("{}", format!("Scanning {}...", root.display()).dimmed());

    // Collect all qualifying files in parallel
    let entries: Vec<(PathBuf, u64)> = WalkDir::new(&root)
        .follow_links(false)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
        .par_bridge()
        .filter_map(|e| {
            let size = e.metadata().ok()?.len();
            if size >= min_bytes {
                Some((e.into_path(), size))
            } else {
                None
            }
        })
        .collect();

    if entries.is_empty() {
        println!(
            "{}",
            format!("No files larger than {} MB found.", min_mb).green()
        );
        return Ok(());
    }

    // Sort by size descending, take top N
    let mut results = entries;
    results.sort_unstable_by_key(|item| std::cmp::Reverse(item.1));
    results.truncate(limit);

    let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("/"));

    let mut table = comfy_table::Table::new();
    table.load_preset(comfy_table::presets::UTF8_BORDERS_ONLY);
    table.set_header(vec!["File", "Size"]);

    for (path, size) in &results {
        let label = path
            .strip_prefix(&home)
            .map(|p| format!("~/{}", p.display()))
            .unwrap_or_else(|_| path.display().to_string());
        table.add_row(vec![label, format_size(*size)]);
    }

    println!(
        "\n{}",
        format!("[ Largest Files in {} ]", root.display())
            .cyan()
            .bold()
    );
    println!("{}", table);
    println!("  Showing top {} files >= {} MB", results.len(), min_mb);

    if trash {
        if dry_run {
            ui::print_warn("Dry run -- nothing moved to Trash.");
            return Ok(());
        }
        if !yes
            && !ui::confirm(
                &format!("Move these {} large file(s) to Trash?", results.len()),
                false,
            )?
        {
            return Ok(());
        }

        let mut analysis = AnalysisResult::default();
        for (path, size) in results {
            analysis.add_with_meta(
                path.display().to_string(),
                path,
                size,
                CleanKind::Unknown,
                RiskLevel::High,
                "Selected from largest-file analysis; review path because this may be original user data.",
            );
        }
        for (path, outcome) in crate::core::trash::trash_clean_items("largest", &analysis.items) {
            match outcome {
                Ok(_) => ui::print_ok(&format!("Moved to Trash: {}", path.display())),
                Err(e) => ui::print_warn(&format!("{}: {}", path.display(), e)),
            }
        }
    }

    Ok(())
}
