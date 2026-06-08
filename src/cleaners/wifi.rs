use anyhow::Result;
use colored::Colorize;
use comfy_table::{Table, presets::UTF8_BORDERS_ONLY};
use crate::core::cmd::run_cmd;

const AIRPORT: &str = "/System/Library/PrivateFrameworks/Apple80211.framework/Versions/Current/Resources/airport";

fn signal_quality(dbm: i64) -> &'static str {
    if dbm >= -50 {
        "Excellent"
    } else if dbm >= -70 {
        "Good"
    } else {
        "Weak"
    }
}

/// Extract a value from airport -I output: "  key: value"
fn airport_val<'a>(lines: &'a [&'a str], key: &str) -> Option<&'a str> {
    for line in lines {
        let trimmed = line.trim();
        if let Some(rest) = trimmed.strip_prefix(&format!("{}:", key)) {
            return Some(rest.trim());
        }
    }
    None
}

pub fn run() -> Result<()> {
    println!("\n{}", "[ Wi-Fi Information ]".cyan().bold());

    let airport_r = run_cmd(&[AIRPORT, "-I"]);
    if airport_r.output.trim().is_empty() || airport_r.output.contains("AirPort: Off") {
        println!("  Wi-Fi is off or airport utility not accessible.");
        return Ok(());
    }

    let lines: Vec<&str> = airport_r.output.lines().collect();

    let ssid      = airport_val(&lines, "SSID").unwrap_or("N/A").to_string();
    let bssid     = airport_val(&lines, "BSSID").unwrap_or("N/A").to_string();
    let channel   = airport_val(&lines, "channel").unwrap_or("N/A").to_string();
    let tx_rate   = airport_val(&lines, "lastTxRate").unwrap_or("N/A").to_string();
    let noise_raw = airport_val(&lines, "noise").unwrap_or("N/A").to_string();

    let signal_raw = airport_val(&lines, "agrCtlRSSI")
        .or_else(|| airport_val(&lines, "RSSI"))
        .unwrap_or("N/A")
        .to_string();

    let signal_label = {
        if let Ok(dbm) = signal_raw.parse::<i64>() {
            format!("{} dBm ({})", dbm, signal_quality(dbm))
        } else {
            signal_raw.clone()
        }
    };

    // Get network interface
    let route_r = run_cmd(&["route", "get", "default"]);
    let iface = route_r
        .output
        .lines()
        .find(|l| l.trim_start().starts_with("interface:"))
        .and_then(|l| l.split_whitespace().nth(1))
        .unwrap_or("N/A")
        .to_string();

    // Get DNS servers
    let dns_r = run_cmd(&["scutil", "--dns"]);
    let mut dns_servers: Vec<String> = Vec::new();
    for line in dns_r.output.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("nameserver[") {
            if let Some(ip) = trimmed.split(':').nth(1) {
                let ip = ip.trim().to_string();
                if !dns_servers.contains(&ip) {
                    dns_servers.push(ip);
                }
            }
        }
    }
    let dns_str = if dns_servers.is_empty() {
        "N/A".to_string()
    } else {
        dns_servers.join(", ")
    };

    let mut table = Table::new();
    table.load_preset(UTF8_BORDERS_ONLY);
    table.set_header(vec!["Field", "Value"]);

    let rows = [
        ("SSID",       ssid),
        ("Interface",  iface),
        ("Signal",     signal_label),
        ("Noise",      noise_raw),
        ("Channel",    channel),
        ("BSSID",      bssid),
        ("TX Rate",    format!("{} Mbps", tx_rate)),
        ("DNS Servers",dns_str),
    ];

    for (k, v) in &rows {
        table.add_row(vec![k.to_string(), v.clone()]);
    }
    println!("{}", table);

    Ok(())
}
