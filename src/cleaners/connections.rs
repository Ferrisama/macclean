use anyhow::Result;
use colored::Colorize;
use comfy_table::{Table, presets::UTF8_BORDERS_ONLY};
use std::collections::HashMap;
use crate::core::cmd::run_cmd;

pub fn run(proc_filter: Option<&str>) -> Result<()> {
    println!("\n{}", "[ Active Network Connections ]".cyan().bold());

    let r = run_cmd(&["lsof", "-i", "-n", "-P", "+c", "0"]);
    if r.output.trim().is_empty() {
        println!("  No connections found.");
        return Ok(());
    }

    // Group: process_name -> Vec<(proto, addr, state)>
    let mut groups: HashMap<String, Vec<(String, String, String)>> = HashMap::new();

    for line in r.output.lines().skip(1) {
        let parts: Vec<&str> = line.split_whitespace().collect();
        // COMMAND PID USER FD TYPE DEVICE SIZE/OFF NODE NAME  (optionally STATE)
        if parts.len() < 9 {
            continue;
        }

        let process = parts[0].to_string();

        // Apply process filter if provided
        if let Some(filter) = proc_filter {
            if !process.to_lowercase().contains(&filter.to_lowercase()) {
                continue;
            }
        }

        let proto = parts[7].to_string();
        let addr  = parts[8].to_string();

        // State is sometimes appended in parentheses at the end: "(ESTABLISHED)"
        let state = parts
            .iter()
            .find(|p| p.starts_with('(') && p.ends_with(')'))
            .map(|s| s.trim_matches(|c| c == '(' || c == ')').to_string())
            .unwrap_or_default();

        groups
            .entry(process)
            .or_default()
            .push((proto, addr, state));
    }

    if groups.is_empty() {
        println!("  No connections found.");
        return Ok(());
    }

    let mut processes: Vec<String> = groups.keys().cloned().collect();
    processes.sort();

    for proc_name in &processes {
        println!("\n  {}", proc_name.cyan().bold());

        let mut table = Table::new();
        table.load_preset(UTF8_BORDERS_ONLY);
        table.set_header(vec!["Proto", "Address", "State"]);

        let conns = &groups[proc_name];
        for (proto, addr, state) in conns {
            let state_colored = match state.as_str() {
                "ESTABLISHED" => state.green().to_string(),
                "LISTEN"      => state.yellow().to_string(),
                ""            => String::new(),
                other         => other.dimmed().to_string(),
            };
            table.add_row(vec![proto.clone(), addr.clone(), state_colored]);
        }
        println!("{}", table);
    }

    Ok(())
}
