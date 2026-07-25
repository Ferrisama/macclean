use crate::core::{AnalysisResult, CleanKind, RiskLevel};
use crate::ui::{self, format_size};
use anyhow::Result;
use colored::Colorize;
use indicatif::{ProgressBar, ProgressStyle};
use rayon::prelude::*;
use sha2::{Digest, Sha256};
use std::cmp::Reverse;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

pub fn run(
    min_mb: u64,
    scan_path: Option<PathBuf>,
    trash: bool,
    keep: &str,
    dry_run: bool,
    yes: bool,
) -> Result<()> {
    let root = scan_path.unwrap_or_else(|| dirs::home_dir().unwrap_or_else(|| PathBuf::from(".")));
    let min_bytes = min_mb * 1024 * 1024;

    println!(
        "{}",
        format!(
            "Scanning {} for duplicates >= {} MB...",
            root.display(),
            min_mb
        )
        .dimmed()
    );

    // ── Step 1: Group files by size (cheap filter) ────────────────────────────
    let mut by_size: HashMap<u64, Vec<PathBuf>> = HashMap::new();

    for entry in WalkDir::new(&root)
        .follow_links(false)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
    {
        if let Ok(meta) = entry.metadata() {
            let size = meta.len();
            if size >= min_bytes {
                by_size.entry(size).or_default().push(entry.into_path());
            }
        }
    }

    // ── Step 2: Parallel hash of candidates ──────────────────────────────────
    let candidates: Vec<(u64, Vec<PathBuf>)> = by_size
        .into_iter()
        .filter(|(_, paths)| paths.len() > 1)
        .collect();

    let total_files: usize = candidates.iter().map(|(_, p)| p.len()).sum();

    if total_files == 0 {
        println!("{}", "No duplicate files found.".green());
        return Ok(());
    }

    let pb = ProgressBar::new(total_files as u64);
    pb.set_style(
        ProgressStyle::with_template(
            "{spinner:.cyan} [{bar:40.cyan/blue}] {pos}/{len} Hashing files...",
        )
        .unwrap()
        .progress_chars("=>-"),
    );

    let mut by_hash: HashMap<String, Vec<(PathBuf, u64)>> = HashMap::new();

    for (size, paths) in &candidates {
        let hashes: Vec<Option<String>> = paths.par_iter().map(|path| hash_file(path)).collect();

        for (path, hash) in paths.iter().zip(hashes) {
            pb.inc(1);
            if let Some(h) = hash {
                by_hash.entry(h).or_default().push((path.clone(), *size));
            }
        }
    }
    pb.finish_and_clear();

    let dup_groups: Vec<Vec<(PathBuf, u64)>> = by_hash
        .into_values()
        .filter(|v| v.len() > 1)
        .map(|mut group| {
            group.sort_by_key(|(path, _)| path.display().to_string());
            group
        })
        .collect();

    if dup_groups.is_empty() {
        println!("{}", "No duplicate files found.".green());
        return Ok(());
    }

    // Sort groups by wasted space descending
    let mut groups = dup_groups;
    groups.sort_by_key(|g| Reverse(g[0].1 * (g.len() as u64 - 1)));

    let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("/"));

    let mut table = comfy_table::Table::new();
    table.load_preset(comfy_table::presets::UTF8_BORDERS_ONLY);
    table.set_header(vec!["Duplicate Files", "Size", "Copies", "Wasted"]);

    let mut total_wasted: u64 = 0;
    for group in &groups {
        let size = group[0].1;
        let wasted = size * (group.len() as u64 - 1);
        total_wasted += wasted;

        let first_path = group[0]
            .0
            .strip_prefix(&home)
            .map(|p| format!("~/{}", p.display()))
            .unwrap_or_else(|_| group[0].0.display().to_string());

        table.add_row(vec![
            first_path,
            format_size(size),
            group.len().to_string(),
            format_size(wasted),
        ]);

        for (path, _) in &group[1..] {
            let label = path
                .strip_prefix(&home)
                .map(|p| format!("  -> ~/{}", p.display()))
                .unwrap_or_else(|_| format!("  -> {}", path.display()));
            table.add_row(vec![label, String::new(), String::new(), String::new()]);
        }
    }

    println!("\n{}", "[ Duplicate Files ]".cyan().bold());
    println!("{}", table);
    println!("  Total wasted: {}", format_size(total_wasted).bold());
    if !trash {
        println!(
            "  {} duplicate group(s). Use --trash to move duplicate copies to Trash.",
            groups.len()
        );
        return Ok(());
    }
    if dry_run {
        ui::print_warn("Dry run -- duplicate files were not moved to Trash.");
        return Ok(());
    }

    let keep = KeepStrategy::parse(keep);
    let mut analysis = AnalysisResult::default();
    for group in &groups {
        let keep_path = keep.select(group);
        for (path, size) in group {
            if path == keep_path {
                continue;
            }
            analysis.add_with_meta(
                path.display().to_string(),
                path.clone(),
                *size,
                CleanKind::Duplicate,
                RiskLevel::High,
                format!(
                    "Content-identical duplicate; keeping {} by {} strategy.",
                    keep_path.display(),
                    keep.label()
                ),
            );
        }
    }

    if !yes
        && !ui::confirm(
            &format!("Move {} duplicate file(s) to Trash?", analysis.items.len()),
            false,
        )?
    {
        return Ok(());
    }

    for (path, outcome) in crate::core::trash::trash_clean_items("dupes", &analysis.items) {
        match outcome {
            Ok(_) => ui::print_ok(&format!("Moved to Trash: {}", path.display())),
            Err(e) => ui::print_warn(&format!("{}: {}", path.display(), e)),
        }
    }

    Ok(())
}

#[derive(Clone, Copy)]
enum KeepStrategy {
    First,
    Newest,
    Oldest,
    ShortestPath,
}

impl KeepStrategy {
    fn parse(value: &str) -> Self {
        match value {
            "newest" => Self::Newest,
            "oldest" => Self::Oldest,
            "shortest" | "shortest-path" => Self::ShortestPath,
            _ => Self::First,
        }
    }

    fn label(self) -> &'static str {
        match self {
            Self::First => "first",
            Self::Newest => "newest",
            Self::Oldest => "oldest",
            Self::ShortestPath => "shortest-path",
        }
    }

    fn select(self, group: &[(PathBuf, u64)]) -> &PathBuf {
        match self {
            Self::First => &group[0].0,
            Self::Newest => {
                &group
                    .iter()
                    .max_by_key(|(path, _)| modified_secs(path))
                    .unwrap_or(&group[0])
                    .0
            }
            Self::Oldest => {
                &group
                    .iter()
                    .min_by_key(|(path, _)| modified_secs(path))
                    .unwrap_or(&group[0])
                    .0
            }
            Self::ShortestPath => {
                &group
                    .iter()
                    .min_by_key(|(path, _)| path.components().count())
                    .unwrap_or(&group[0])
                    .0
            }
        }
    }
}

fn modified_secs(path: &Path) -> u64 {
    path.metadata()
        .ok()
        .and_then(|m| m.modified().ok())
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

fn hash_file(path: &Path) -> Option<String> {
    let mut file = std::fs::File::open(path).ok()?;
    let mut hasher = Sha256::new();
    std::io::copy(&mut file, &mut hasher).ok()?;
    Some(format!("{:x}", hasher.finalize()))
}
