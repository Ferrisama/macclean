use anyhow::Result;
use colored::Colorize;
use comfy_table::{Table, presets::UTF8_BORDERS_ONLY};
use std::path::{Path, PathBuf};

/// Extract the first `<string>...</string>` that follows a `<key>KEY</key>` in XML plist text.
fn extract_after_key(xml: &str, key: &str) -> Option<String> {
    let key_tag = format!("<key>{}</key>", key);
    let pos = xml.find(&key_tag)?;
    let rest = &xml[pos + key_tag.len()..];
    let start = rest.find("<string>")? + "<string>".len();
    let end   = rest[start..].find("</string>")?;
    Some(rest[start..start + end].trim().to_string())
}

/// Find the binary path: try `Program` key first, then first entry of `ProgramArguments`.
fn extract_program(xml: &str) -> Option<String> {
    if let Some(p) = extract_after_key(xml, "Program") {
        if !p.is_empty() {
            return Some(p);
        }
    }
    // ProgramArguments → array of strings; first <string> after the key
    let tag = "<key>ProgramArguments</key>";
    let pos = xml.find(tag)?;
    let rest = &xml[pos + tag.len()..];
    // skip <array> tag
    let start = rest.find("<string>")? + "<string>".len();
    let end   = rest[start..].find("</string>")?;
    Some(rest[start..start + end].trim().to_string())
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
    let home_str  = dirs::home_dir()
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

        let xml = match std::fs::read_to_string(&path) {
            Ok(s) => s,
            Err(_) => {
                // Binary plist — skip gracefully
                continue;
            }
        };

        // Quick binary plist check: starts with "bplist"
        if xml.trim_start().starts_with("bplist") {
            continue;
        }

        let label   = extract_after_key(&xml, "Label")
            .unwrap_or_else(|| path.file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("unknown")
                .to_string());

        let program = extract_program(&xml);

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
