use anyhow::Result;
use colored::Colorize;
use comfy_table::{Table, presets::UTF8_BORDERS_ONLY};
use crate::core::cmd::run_cmd;
use crate::core::fs::dir_size;
use crate::ui::format_size;

fn filevault_status() -> bool {
    run_cmd(&["fdesetup", "status"]).output.contains("FileVault is On")
}

fn firewall_status() -> bool {
    let r = run_cmd(&[
        "/usr/libexec/ApplicationFirewall/socketfilterfw",
        "--getglobalstate",
    ]);
    r.output.to_lowercase().contains("enabled")
}

fn sip_status() -> bool {
    let r = run_cmd(&["csrutil", "status"]);
    r.output.to_lowercase().contains("enabled")
}

pub fn run() -> Result<()> {
    println!("\n{}", "[ System Health Snapshot ]".cyan().bold());

    // ── Disk ──────────────────────────────────────────────────────────────────
    let df_r = run_cmd(&["df", "-k", "/"]);
    let (disk_total, disk_used, disk_free) = {
        let mut t = (0u64, 0u64, 0u64);
        for line in df_r.output.lines().skip(1) {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() >= 4 {
                let blocks: u64 = parts[1].parse().unwrap_or(0);
                let used: u64   = parts[2].parse().unwrap_or(0);
                let avail: u64  = parts[3].parse().unwrap_or(0);
                t = (blocks * 1024, used * 1024, avail * 1024);
                break;
            }
        }
        t
    };

    // ── Memory ────────────────────────────────────────────────────────────────
    let total_r = run_cmd(&["sysctl", "-n", "hw.memsize"]);
    let mem_total: u64 = total_r.output.trim().parse().unwrap_or(0);

    let page_size_r = run_cmd(&["sysctl", "-n", "vm.pagesize"]);
    let page_size: u64 = page_size_r.output.trim().parse().unwrap_or(4096);

    let vm_r = run_cmd(&["vm_stat"]);
    let mut pages_active: u64 = 0;
    let mut pages_speculative: u64 = 0;
    let mut pages_wired: u64 = 0;
    let mut pages_inactive: u64 = 0;

    for line in vm_r.output.lines() {
        let trimmed = line.trim_start();
        let parse_val = |l: &str| -> u64 {
            l.split(':')
                .nth(1)
                .unwrap_or("0")
                .trim()
                .trim_end_matches('.')
                .parse()
                .unwrap_or(0)
        };
        if trimmed.starts_with("Pages active:") {
            pages_active = parse_val(trimmed);
        } else if trimmed.starts_with("Pages speculative:") {
            pages_speculative = parse_val(trimmed);
        } else if trimmed.starts_with("Pages wired down:") {
            pages_wired = parse_val(trimmed);
        } else if trimmed.starts_with("Pages inactive:") {
            pages_inactive = parse_val(trimmed);
        }
    }
    let mem_used = (pages_active + pages_speculative + pages_wired) * page_size;
    // pages_inactive is parsed but not shown in the snapshot summary
    let _ = pages_inactive;

    // ── CPU ───────────────────────────────────────────────────────────────────
    let ncpu_r = run_cmd(&["sysctl", "-n", "hw.ncpu"]);
    let ncpu = ncpu_r.output.trim().to_string();

    let load_r = run_cmd(&["sysctl", "-n", "vm.loadavg"]);
    // output: "{ 1.23 0.98 0.87 }"
    let load_str = load_r.output.trim().to_string();

    // ── Battery ───────────────────────────────────────────────────────────────
    let batt_r = run_cmd(&["pmset", "-g", "batt"]);
    let battery_line = batt_r
        .output
        .lines()
        .find(|l| l.contains('%'))
        .unwrap_or("N/A")
        .trim()
        .to_string();

    // ── Print table ───────────────────────────────────────────────────────────
    let mut table = Table::new();
    table.load_preset(UTF8_BORDERS_ONLY);
    table.set_header(vec!["Category", "Info"]);

    // Disk
    let disk_pct = if disk_total > 0 { disk_used * 100 / disk_total } else { 0 };
    table.add_row(vec![
        "Disk (/)".to_string(),
        format!(
            "{} used / {} total ({} free) - {}%",
            format_size(disk_used),
            format_size(disk_total),
            format_size(disk_free),
            disk_pct
        ),
    ]);

    // Memory
    let mem_pct = if mem_total > 0 { mem_used * 100 / mem_total } else { 0 };
    table.add_row(vec![
        "Memory".to_string(),
        format!(
            "{} used / {} total - {}%",
            format_size(mem_used),
            format_size(mem_total),
            mem_pct
        ),
    ]);

    // CPU
    table.add_row(vec![
        "CPU".to_string(),
        format!("{} logical cores  |  Load avg: {}", ncpu, load_str),
    ]);

    // Battery
    table.add_row(vec!["Battery".to_string(), battery_line]);

    println!("{}", table);

    // ── Security summary ──────────────────────────────────────────────────────
    println!("\n{}", "[ Security Summary ]".cyan().bold());
    let mut sec_table = Table::new();
    sec_table.load_preset(UTF8_BORDERS_ONLY);
    sec_table.set_header(vec!["Check", "Status"]);

    let checks = [
        ("FileVault",  filevault_status()),
        ("Firewall",   firewall_status()),
        ("SIP",        sip_status()),
    ];
    for (name, ok) in &checks {
        let status = if *ok {
            "OK".green().to_string()
        } else {
            "OFF".red().to_string()
        };
        sec_table.add_row(vec![name.to_string(), status]);
    }
    println!("{}", sec_table);

    // ── Top space users in home ───────────────────────────────────────────────
    let home = dirs::home_dir().unwrap_or_else(|| std::path::PathBuf::from("."));

    let targets = [
        ("Gradle",       ".gradle/caches"),
        ("Xcode Dev",    "Library/Developer"),
        ("Containers",   "Library/Containers"),
        ("App Support",  "Library/Application Support"),
        ("Caches",       "Library/Caches"),
        ("npm",          ".npm"),
        ("cargo",        ".cargo/registry"),
    ];

    let mut sizes: Vec<(&str, u64)> = targets
        .iter()
        .map(|(label, rel)| {
            let p = home.join(rel);
            let sz = if p.exists() { dir_size(&p) } else { 0 };
            (*label, sz)
        })
        .collect();
    sizes.sort_by_key(|(_, s)| std::cmp::Reverse(*s));

    println!("\n{}", "[ Top Space Users (Home) ]".cyan().bold());
    let mut sz_table = Table::new();
    sz_table.load_preset(UTF8_BORDERS_ONLY);
    sz_table.set_header(vec!["Location", "Size"]);
    for (label, sz) in sizes.iter().take(6) {
        sz_table.add_row(vec![label.to_string(), format_size(*sz)]);
    }
    println!("{}", sz_table);

    Ok(())
}
