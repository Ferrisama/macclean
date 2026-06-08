use std::collections::HashSet;
use std::path::PathBuf;
use anyhow::Result;
use crate::core::{AnalysisResult, Cleaner, CleanItem};
use crate::core::fs::dir_size;
use crate::ui;

pub struct ZshCleaner;

impl Cleaner for ZshCleaner {
    fn name(&self) -> &str { "zsh" }
    fn display_name(&self) -> &str { "ZSH History & Caches" }

    fn analyze(&self) -> Result<AnalysisResult> {
        let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("/tmp"));
        let mut result = AnalysisResult::default();

        // Check ~/.zsh_history for duplicates
        let history_path = home.join(".zsh_history");
        if history_path.exists() {
            if let Ok(contents) = std::fs::read_to_string(&history_path) {
                let original_size = contents.len() as u64;
                let deduped = dedup_zsh_history(&contents);
                let deduped_size = deduped.len() as u64;
                let saved = original_size.saturating_sub(deduped_size);

                let total_lines = contents.lines().count();
                let deduped_lines = deduped.lines().count();
                let duplicates = total_lines.saturating_sub(deduped_lines);

                if duplicates > 0 {
                    result.items.push(CleanItem {
                        label: format!("{} duplicate history entries", duplicates),
                        path: history_path,
                        size_bytes: saved,
                        removable: true,
                    });
                }
            }
        }

        // Check ~/.zcompcache
        let zcompcache = home.join(".zcompcache");
        if zcompcache.exists() {
            let size = dir_size(&zcompcache);
            result.add("ZSH completion cache (~/.zcompcache)", zcompcache, size);
        }

        // Check ~/.zcompdump* files
        let home_entries = match std::fs::read_dir(&home) {
            Ok(e) => e,
            Err(_) => return Ok(result),
        };
        for entry in home_entries.filter_map(|e| e.ok()) {
            let name = entry.file_name().to_string_lossy().to_string();
            if name.starts_with(".zcompdump") {
                let path = entry.path();
                let size = path.metadata().map(|m| m.len()).unwrap_or(0);
                result.add(format!("ZSH completion dump ({})", name), path, size);
            }
        }

        Ok(result)
    }

    fn clean(&self, result: &AnalysisResult, dry_run: bool, yes: bool) -> Result<()> {
        if result.items.is_empty() {
            println!("Nothing to clean in ZSH history/caches.");
            return Ok(());
        }
        ui::print_analysis("ZSH History & Caches", &result.items);
        if dry_run { return Ok(()); }
        if !yes && !ui::confirm("Clean ZSH history and caches?", false)? { return Ok(()); }

        let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("/tmp"));

        for item in &result.items {
            // History dedup
            if item.path == home.join(".zsh_history") {
                match dedup_and_write_history(&item.path) {
                    Ok(_) => ui::print_ok("Deduplicated ZSH history"),
                    Err(e) => ui::print_warn(&format!("ZSH history: {}", e)),
                }
                continue;
            }

            // Completion cache dir
            if item.path.is_dir() {
                match std::fs::remove_dir_all(&item.path) {
                    Ok(_) => ui::print_ok(&format!("Removed {}", item.label)),
                    Err(e) => ui::print_warn(&format!("{}: {}", item.label, e)),
                }
            } else {
                // Completion dump files
                match std::fs::remove_file(&item.path) {
                    Ok(_) => ui::print_ok(&format!("Removed {}", item.label)),
                    Err(e) => ui::print_warn(&format!("{}: {}", item.label, e)),
                }
            }
        }
        Ok(())
    }
}

/// Deduplicate zsh history lines, preserving order (last occurrence wins for extended history).
/// Handles both plain lines and extended_history format (": timestamp:elapsed;command").
fn dedup_zsh_history(contents: &str) -> String {
    let mut seen: HashSet<String> = HashSet::new();

    // Walk lines in reverse to keep the last occurrence of each command
    let lines: Vec<&str> = contents.lines().collect();
    let mut result_rev: Vec<&str> = Vec::new();

    for line in lines.iter().rev() {
        // Extract the command part for dedup key
        let key = if line.starts_with(": ") {
            // extended_history format: ": timestamp:elapsed;command"
            line.splitn(2, ';').nth(1).unwrap_or(line).trim()
        } else {
            line.trim()
        };

        if seen.insert(key.to_string()) {
            result_rev.push(line);
        }
    }

    result_rev.reverse();
    let kept = result_rev;

    let mut out = kept.join("\n");
    if !out.is_empty() && contents.ends_with('\n') {
        out.push('\n');
    }
    out
}

fn dedup_and_write_history(path: &std::path::Path) -> Result<()> {
    let contents = std::fs::read_to_string(path)?;
    let deduped = dedup_zsh_history(&contents);
    std::fs::write(path, deduped.as_bytes())?;
    Ok(())
}
