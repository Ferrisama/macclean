use anyhow::Result;
use colored::Colorize;
use comfy_table::{Table, presets::UTF8_BORDERS_ONLY};
use std::collections::HashSet;
use crate::core::cmd::run_cmd;

pub fn run() -> Result<()> {
    println!("\n{}", "[ Listening Ports ]".cyan().bold());

    let r = run_cmd(&["lsof", "-i", "-n", "-P", "-sTCP:LISTEN"]);
    if r.output.trim().is_empty() {
        println!("  No listening ports found.");
        return Ok(());
    }

    // (process, pid, addr, proto)
    let mut rows: Vec<(String, String, String, String)> = Vec::new();
    let mut seen: HashSet<(String, String)> = HashSet::new();

    for line in r.output.lines().skip(1) {
        let parts: Vec<&str> = line.split_whitespace().collect();
        // COMMAND PID USER FD TYPE DEVICE SIZE/OFF NODE NAME
        //   0      1   2   3   4    5       6       7    8
        if parts.len() < 9 {
            continue;
        }
        let process = parts[0].to_string();
        let pid     = parts[1].to_string();
        let proto   = parts[7].to_string();
        let addr    = parts[8].to_string();

        let key = (process.clone(), addr.clone());
        if seen.contains(&key) {
            continue;
        }
        seen.insert(key);
        rows.push((process, pid, addr, proto));
    }

    if rows.is_empty() {
        println!("  No listening ports found.");
        return Ok(());
    }

    rows.sort_by(|a, b| a.0.cmp(&b.0).then(a.2.cmp(&b.2)));

    let mut table = Table::new();
    table.load_preset(UTF8_BORDERS_ONLY);
    table.set_header(vec!["Process", "PID", "Address/Port", "Proto"]);

    for (process, pid, addr, proto) in &rows {
        table.add_row(vec![
            process.clone(),
            pid.clone(),
            addr.clone(),
            proto.clone(),
        ]);
    }
    println!("{}", table);
    println!("  {} listening port(s)", rows.len());

    Ok(())
}
