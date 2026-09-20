use crate::core::{AnalysisResult, CleanKind, RiskLevel};
use crate::ui::{self, format_size};
use anyhow::Result;
use colored::Colorize;
use indicatif::{ProgressBar, ProgressStyle};
use rayon::prelude::*;
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::cmp::Reverse;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

#[derive(Debug, Clone, Serialize)]
pub struct DuplicateFile {
    pub path: PathBuf,
    pub size_bytes: u64,
    pub modified_at: u64,
}

#[derive(Debug, Clone, Serialize)]
pub struct DuplicateGroup {
    pub id: String,
    pub size_bytes: u64,
    pub wasted_bytes: u64,
    pub files: Vec<DuplicateFile>,
}

#[derive(Debug, Clone, Serialize)]
pub struct DuplicateReport {
    pub schema_version: u32,
    pub root: PathBuf,
    pub min_bytes: u64,
    pub scanned_files: u64,
    pub hashed_files: u64,
    pub partial: bool,
    pub error_count: u64,
    pub total_wasted_bytes: u64,
    pub groups: Vec<DuplicateGroup>,
}

pub fn analyze(root: &Path, min_bytes: u64) -> Result<DuplicateReport> {
    let root = std::fs::canonicalize(root)?;
    if !root.is_dir() {
        anyhow::bail!("Duplicate scan root is not a directory: {}", root.display());
    }

    let mut by_size: HashMap<u64, Vec<PathBuf>> = HashMap::new();
    let mut scanned_files = 0_u64;
    let mut error_count = 0_u64;
    for entry in WalkDir::new(&root).follow_links(false) {
        let entry = match entry {
            Ok(entry) => entry,
            Err(_) => {
                error_count += 1;
                continue;
            }
        };
        if !entry.file_type().is_file() {
            continue;
        }
        scanned_files += 1;
        match entry.metadata() {
            Ok(metadata) if metadata.len() >= min_bytes => {
                by_size
                    .entry(metadata.len())
                    .or_default()
                    .push(entry.into_path());
            }
            Ok(_) => {}
            Err(_) => error_count += 1,
        }
    }

    let candidates: Vec<_> = by_size
        .into_iter()
        .filter(|(_, paths)| paths.len() > 1)
        .collect();
    let hashed: Vec<_> = candidates
        .par_iter()
        .flat_map_iter(|(size, paths)| {
            paths
                .iter()
                .map(|path| (path.clone(), *size, hash_file(path)))
        })
        .collect();
    let mut by_hash: HashMap<String, Vec<(PathBuf, u64)>> = HashMap::new();
    let mut hashed_files = 0_u64;
    for (path, size, hash) in hashed {
        if let Some(hash) = hash {
            hashed_files += 1;
            by_hash.entry(hash).or_default().push((path, size));
        } else {
            error_count += 1;
        }
    }

    let mut groups: Vec<DuplicateGroup> = by_hash
        .into_iter()
        .filter(|(_, files)| files.len() > 1)
        .map(|(id, mut files)| {
            files.sort_by(|a, b| a.0.cmp(&b.0));
            let size_bytes = files[0].1;
            DuplicateGroup {
                id,
                size_bytes,
                wasted_bytes: size_bytes.saturating_mul(files.len().saturating_sub(1) as u64),
                files: files
                    .into_iter()
                    .map(|(path, size_bytes)| DuplicateFile {
                        modified_at: modified_secs(&path),
                        path,
                        size_bytes,
                    })
                    .collect(),
            }
        })
        .collect();
    groups.sort_by_key(|group| Reverse(group.wasted_bytes));
    let total_wasted_bytes = groups.iter().fold(0_u64, |total, group| {
        total.saturating_add(group.wasted_bytes)
    });

    Ok(DuplicateReport {
        schema_version: 1,
        root,
        min_bytes,
        scanned_files,
        hashed_files,
        partial: error_count > 0,
        error_count,
        total_wasted_bytes,
        groups,
    })
}

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

    let pb = ProgressBar::new_spinner();
    pb.set_style(
        ProgressStyle::with_template("{spinner:.cyan} Scanning and hashing files...")
            .unwrap()
            .progress_chars("=>-"),
    );

    pb.enable_steady_tick(std::time::Duration::from_millis(100));
    let report = analyze(&root, min_bytes)?;
    pb.finish_and_clear();

    if report.groups.is_empty() {
        println!("{}", "No duplicate files found.".green());
        return Ok(());
    }

    let groups: Vec<Vec<(PathBuf, u64)>> = report
        .groups
        .iter()
        .map(|group| {
            group
                .files
                .iter()
                .map(|file| (file.path.clone(), file.size_bytes))
                .collect()
        })
        .collect();

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

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn analysis_groups_only_content_identical_files() {
        let dir = tempdir().unwrap();
        std::fs::write(dir.path().join("a.bin"), b"same bytes").unwrap();
        std::fs::write(dir.path().join("b.bin"), b"same bytes").unwrap();
        std::fs::write(dir.path().join("c.bin"), b"different!").unwrap();

        let report = analyze(dir.path(), 1).unwrap();

        assert_eq!(report.scanned_files, 3);
        assert_eq!(report.groups.len(), 1);
        assert_eq!(report.groups[0].files.len(), 2);
        assert_eq!(report.groups[0].wasted_bytes, 10);
        assert!(!report.partial);
    }

    #[test]
    fn analysis_respects_minimum_size() {
        let dir = tempdir().unwrap();
        std::fs::write(dir.path().join("a.bin"), b"same").unwrap();
        std::fs::write(dir.path().join("b.bin"), b"same").unwrap();

        let report = analyze(dir.path(), 5).unwrap();

        assert!(report.groups.is_empty());
        assert_eq!(report.hashed_files, 0);
    }
}
