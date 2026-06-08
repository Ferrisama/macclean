use anyhow::Result;
use colored::Colorize;
use comfy_table::{Table, presets::UTF8_BORDERS_ONLY};
use std::path::PathBuf;
use crate::core::cmd::run_cmd;

/// Parse sfltool dumpbtm output.
/// Lines may look like:
///   name = "App Name"
///   url  = file:///Applications/App.app
fn parse_sfltool(output: &str) -> Vec<(String, String)> {
    let mut items: Vec<(String, String)> = Vec::new();
    let mut current_name = String::new();
    let mut current_url  = String::new();

    for line in output.lines() {
        let trimmed = line.trim();
        if let Some(rest) = trimmed.strip_prefix("name = ") {
            current_name = rest.trim_matches('"').to_string();
        } else if let Some(rest) = trimmed.strip_prefix("url = ") {
            current_url = rest
                .trim_matches('"')
                .trim_start_matches("file://")
                .to_string();
        }

        // When we have both, emit and reset
        if !current_name.is_empty() && !current_url.is_empty() {
            items.push((current_name.clone(), current_url.clone()));
            current_name.clear();
            current_url.clear();
        }
    }

    // Flush partial entry (name without url)
    if !current_name.is_empty() && current_url.is_empty() {
        items.push((current_name, String::new()));
    }

    items
}

pub fn run() -> Result<()> {
    println!("\n{}", "[ Login Items ]".cyan().bold());

    let mut items: Vec<(String, String)> = Vec::new();

    // Try sfltool first (macOS 13+)
    let sfltool_r = run_cmd(&["sfltool", "dumpbtm"]);
    if sfltool_r.success() && !sfltool_r.output.trim().is_empty() {
        items = parse_sfltool(&sfltool_r.output);
    }

    // Fallback: osascript
    if items.is_empty() {
        let osa_r = run_cmd(&[
            "osascript",
            "-e",
            "tell application \"System Events\" to get name of every login item",
        ]);
        if osa_r.success() && !osa_r.output.trim().is_empty() {
            for name in osa_r.output.split(',') {
                let n = name.trim().to_string();
                if !n.is_empty() {
                    items.push((n, String::new()));
                }
            }
        }
    }

    if items.is_empty() {
        println!("  No login items found (or not accessible).");
        return Ok(());
    }

    let mut table = Table::new();
    table.load_preset(UTF8_BORDERS_ONLY);
    table.set_header(vec!["App Name", "Path", "Exists?"]);

    for (name, path) in &items {
        let exists = if path.is_empty() {
            // Try to locate by name in common locations
            let candidates = [
                format!("/Applications/{}.app", name),
                format!("/Applications/{}", name),
                dirs::home_dir()
                    .map(|h| h.join("Applications").join(format!("{}.app", name))
                              .to_string_lossy().to_string())
                    .unwrap_or_default(),
            ];
            if candidates.iter().any(|c| !c.is_empty() && PathBuf::from(c).exists()) {
                "Yes".green().to_string()
            } else {
                "?".yellow().to_string()
            }
        } else {
            if PathBuf::from(path).exists() {
                "Yes".green().to_string()
            } else {
                "No".red().to_string()
            }
        };

        table.add_row(vec![
            name.clone(),
            if path.is_empty() { "N/A".to_string() } else { path.clone() },
            exists,
        ]);
    }
    println!("{}", table);

    Ok(())
}
