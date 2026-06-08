use anyhow::Result;
use colored::Colorize;
use comfy_table::{Table, presets::UTF8_BORDERS_ONLY};
use crate::core::cmd::run_cmd;
use crate::cleaners::security::map_service;

fn map_auth(v: &str) -> &'static str {
    match v.trim() {
        "0" => "Denied",
        "1" => "Unknown",
        "2" => "Allowed",
        "3" => "Limited",
        _   => "Unknown",
    }
}

fn query_tcc(label: &str, db_path: &std::path::Path) {
    println!("\n{}", format!("[ App Permissions — {} ]", label).cyan().bold());

    if !db_path.exists() {
        println!("  Database not accessible: {}", db_path.display());
        return;
    }

    let db_str = db_path.to_string_lossy();
    let out = run_cmd(&[
        "sqlite3",
        "-separator",
        "|",
        &db_str,
        "SELECT service,client,auth_value FROM access ORDER BY service,client",
    ]);

    if !out.success() || out.output.trim().is_empty() {
        println!("  No entries found (or database not readable).");
        return;
    }

    let mut table = Table::new();
    table.load_preset(UTF8_BORDERS_ONLY);
    table.set_header(vec!["Permission", "App", "Status"]);

    for line in out.output.lines() {
        let parts: Vec<&str> = line.splitn(3, '|').collect();
        if parts.len() < 3 {
            continue;
        }
        let service = map_service(parts[0].trim());
        let client  = parts[1].trim();
        let status  = map_auth(parts[2].trim());

        let status_colored = match status {
            "Allowed" => status.green().to_string(),
            "Denied"  => status.red().to_string(),
            _         => status.yellow().to_string(),
        };

        table.add_row(vec![
            service.to_string(),
            client.to_string(),
            status_colored,
        ]);
    }
    println!("{}", table);
}

pub fn run() -> Result<()> {
    let home = dirs::home_dir().unwrap_or_else(|| std::path::PathBuf::from("."));

    let user_db = home.join("Library/Application Support/com.apple.TCC/TCC.db");
    let sys_db  = std::path::PathBuf::from("/Library/Application Support/com.apple.TCC/TCC.db");

    query_tcc("User", &user_db);
    query_tcc("System", &sys_db);

    Ok(())
}
