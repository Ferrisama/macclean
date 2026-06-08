use anyhow::Result;
use colored::Colorize;
use comfy_table::{Table, presets::UTF8_BORDERS_ONLY};
use crate::core::cmd::run_cmd;

pub fn map_service(service: &str) -> String {
    match service {
        "kTCCServiceCamera"              => "Camera".into(),
        "kTCCServiceMicrophone"          => "Microphone".into(),
        "kTCCServiceScreenCapture"       => "Screen Recording".into(),
        "kTCCServiceLocation"            => "Location".into(),
        "kTCCServiceAddressBook"         => "Contacts".into(),
        "kTCCServiceCalendar"            => "Calendar".into(),
        "kTCCServicePhotos"              => "Photos".into(),
        "kTCCServiceSystemPolicyAllFiles"=> "Full Disk Access".into(),
        "kTCCServiceAccessibility"       => "Accessibility".into(),
        "kTCCServiceReminders"           => "Reminders".into(),
        "kTCCServiceUbiquity"            => "iCloud".into(),
        "kTCCServiceShareKit"            => "Sharing".into(),
        other => other.replace("kTCCService", ""),
    }
}

pub fn run() -> Result<()> {
    println!("\n{}", "[ Security Checks ]".cyan().bold());

    // ── Security checks table ─────────────────────────────────────────────────
    let fv = run_cmd(&["fdesetup", "status"])
        .output
        .contains("FileVault is On");

    let fw = {
        let r = run_cmd(&[
            "/usr/libexec/ApplicationFirewall/socketfilterfw",
            "--getglobalstate",
        ]);
        r.output.to_lowercase().contains("enabled")
    };

    let sip = {
        let r = run_cmd(&["csrutil", "status"]);
        r.output.to_lowercase().contains("enabled")
    };

    let gk = {
        let r = run_cmd(&["spctl", "--status"]);
        let out = r.output.to_lowercase();
        out.contains("assessments enabled") || out.contains("enabled")
    };

    let mut table = Table::new();
    table.load_preset(UTF8_BORDERS_ONLY);
    table.set_header(vec!["Security Check", "Status"]);

    for (name, ok) in &[
        ("FileVault",  fv),
        ("Firewall",   fw),
        ("SIP",        sip),
        ("Gatekeeper", gk),
    ] {
        let status = if *ok {
            "OK".green().to_string()
        } else {
            "OFF".red().to_string()
        };
        table.add_row(vec![name.to_string(), status]);
    }
    println!("{}", table);

    // ── TCC permissions table ─────────────────────────────────────────────────
    println!("\n{}", "[ App Permissions (TCC) ]".cyan().bold());

    let home = dirs::home_dir().unwrap_or_else(|| std::path::PathBuf::from("."));
    let db_path = home
        .join("Library/Application Support/com.apple.TCC/TCC.db");

    if !db_path.exists() {
        println!("  TCC database not accessible (requires Full Disk Access).");
        return Ok(());
    }

    let db_str = db_path.to_string_lossy();
    let out = run_cmd(&[
        "sqlite3",
        "-separator",
        "|",
        &db_str,
        "SELECT service,client,auth_value FROM access WHERE auth_value=2",
    ]);

    if !out.success() || out.output.trim().is_empty() {
        println!("  No granted permissions found (or TCC.db not readable).");
        return Ok(());
    }

    let mut perm_table = Table::new();
    perm_table.load_preset(UTF8_BORDERS_ONLY);
    perm_table.set_header(vec!["Permission", "App", "Status"]);

    for line in out.output.lines() {
        let parts: Vec<&str> = line.splitn(3, '|').collect();
        if parts.len() < 3 {
            continue;
        }
        let service = map_service(parts[0].trim());
        let client  = parts[1].trim();
        perm_table.add_row(vec![
            service.to_string(),
            client.to_string(),
            "Allowed".green().to_string(),
        ]);
    }
    println!("{}", perm_table);

    Ok(())
}
