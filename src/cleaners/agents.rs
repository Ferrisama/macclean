use anyhow::Result;
use colored::Colorize;
use comfy_table::{presets::UTF8_BORDERS_ONLY, Table};
use std::path::{Path, PathBuf};

/// Read `Label` and program path (`Program`, falling back to the first
/// `ProgramArguments` entry) from a launchd plist. Handles both XML and
/// binary plists -- binary-encoded launchd plists are a known persistence
/// trick and must not be silently skipped by a security-checking command.
fn read_agent_info(path: &Path) -> Option<(Option<String>, Option<String>)> {
    let dict = plist::Value::from_file(path).ok()?.into_dictionary()?;

    let label = dict
        .get("Label")
        .and_then(|v| v.as_string())
        .map(|s| s.to_string());

    let program = dict
        .get("Program")
        .and_then(|v| v.as_string())
        .map(|s| s.to_string())
        .or_else(|| {
            dict.get("ProgramArguments")
                .and_then(|v| v.as_array())
                .and_then(|a| a.first())
                .and_then(|v| v.as_string())
                .map(|s| s.to_string())
        });

    Some((label, program))
}

fn scan_dir(dir: &Path, table: &mut Table) {
    if !dir.exists() {
        return;
    }
    let entries = match std::fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return,
    };

    let dir_label = dir.display().to_string();
    let home_str = dirs::home_dir()
        .map(|h| h.to_string_lossy().to_string())
        .unwrap_or_default();
    let display_dir = if dir_label.starts_with(&home_str) {
        format!("~{}", &dir_label[home_str.len()..])
    } else {
        dir_label
    };

    let mut found_any = false;
    for entry in entries.filter_map(|e| e.ok()) {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("plist") {
            continue;
        }

        let Some((label, program)) = read_agent_info(&path) else {
            continue;
        };

        let label = label.unwrap_or_else(|| {
            path.file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("unknown")
                .to_string()
        });

        let status = match &program {
            None => "No program key".yellow().to_string(),
            Some(prog) => {
                if PathBuf::from(prog).exists() {
                    "OK".green().to_string()
                } else {
                    "Binary missing".red().to_string()
                }
            }
        };

        if !found_any {
            // Section header row
            table.add_row(vec![
                format!("-- {} --", display_dir),
                String::new(),
                String::new(),
            ]);
            found_any = true;
        }

        table.add_row(vec![
            label,
            program.unwrap_or_else(|| "N/A".to_string()),
            status,
        ]);
    }
}

pub fn run() -> Result<()> {
    println!("\n{}", "[ Launch Agents & Daemons ]".cyan().bold());

    let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));

    let dirs_to_scan: &[PathBuf] = &[
        home.join("Library/LaunchAgents"),
        PathBuf::from("/Library/LaunchAgents"),
        PathBuf::from("/Library/LaunchDaemons"),
    ];

    let mut table = Table::new();
    table.load_preset(UTF8_BORDERS_ONLY);
    table.set_header(vec!["Label", "Program", "Status"]);

    for d in dirs_to_scan {
        scan_dir(d, &mut table);
    }

    println!("{}", table);

    Ok(())
}
