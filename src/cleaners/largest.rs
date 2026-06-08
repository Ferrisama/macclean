use std::path::PathBuf;
use anyhow::Result;
use walkdir::WalkDir;
use rayon::prelude::*;
use rayon::iter::ParallelBridge;
use colored::Colorize;
use crate::ui::format_size;

pub fn run(min_mb: u64, limit: usize, scan_path: Option<PathBuf>) -> Result<()> {
    let root = scan_path
        .unwrap_or_else(|| dirs::home_dir().unwrap_or_else(|| PathBuf::from(".")));
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
    results.sort_unstable_by(|a, b| b.1.cmp(&a.1));
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
    println!(
        "  Showing top {} files >= {} MB",
        results.len(),
        min_mb
    );

    Ok(())
}
