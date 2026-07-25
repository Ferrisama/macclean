use colored::Colorize;
use comfy_table::{presets::UTF8_BORDERS_ONLY, Table};

use crate::core::CleanItem;

pub fn format_size(n: u64) -> String {
    let units = ["B", "KB", "MB", "GB", "TB"];
    let mut size = n as f64;
    for unit in &units {
        if size < 1024.0 {
            return format!("{:.1} {}", size, unit);
        }
        size /= 1024.0;
    }
    format!("{:.1} PB", size)
}

pub fn confirm(prompt: &str, default: bool) -> anyhow::Result<bool> {
    use std::io::Write;
    let hint = if default { "[Y/n]" } else { "[y/N]" };
    print!("{} {} ", prompt, hint);
    std::io::stdout().flush()?;
    let mut input = String::new();
    std::io::stdin().read_line(&mut input)?;
    let ans = input.trim().to_lowercase();
    if ans.is_empty() {
        return Ok(default);
    }
    Ok(ans == "y" || ans == "yes")
}

pub fn print_analysis(title: &str, items: &[CleanItem]) {
    let mut table = Table::new();
    table.load_preset(UTF8_BORDERS_ONLY);
    table.set_header(vec!["Location", "Size", "Type", "Risk", "Why"]);
    for item in items {
        table.add_row(vec![
            item.label.clone(),
            format_size(item.size_bytes),
            item.kind.label().to_string(),
            item.risk.label().to_string(),
            item.reason.clone(),
        ]);
    }
    let total: u64 = items
        .iter()
        .filter(|i| i.removable)
        .map(|i| i.size_bytes)
        .sum();
    println!();
    println!("{}", format!("[ {} ]", title).cyan().bold());
    println!("{}", table);
    println!("  Total recoverable: {}", format_size(total).bold());
}

pub fn print_history(limit: usize) -> anyhow::Result<()> {
    let mut records = crate::core::history::read_all()?;
    records.sort_by_key(|record| std::cmp::Reverse(record.timestamp));

    if records.is_empty() {
        println!("No macclean history found.");
        return Ok(());
    }

    let mut table = Table::new();
    table.load_preset(UTF8_BORDERS_ONLY);
    table.set_header(vec![
        "Session",
        "Cleaner",
        "Item",
        "Size",
        "Method",
        "Original Path",
    ]);
    for record in records.into_iter().take(limit) {
        table.add_row(vec![
            record.session_id,
            record.cleaner,
            record.label,
            format_size(record.size_bytes),
            record.method,
            record.original_path.display().to_string(),
        ]);
    }

    println!("\n{}", "[ Cleanup History ]".cyan().bold());
    println!("{}", table);
    println!("  Restore latest: macclean restore");
    println!("  Restore session: macclean restore <session>");
    Ok(())
}

pub fn print_plan(plan: &crate::core::plan::StoredPlan) {
    let mut table = Table::new();
    table.load_preset(UTF8_BORDERS_ONLY);
    table.set_header(vec!["Path", "Size", "Type", "Risk", "Why"]);
    for item in &plan.items {
        table.add_row(vec![
            item.path.display().to_string(),
            format_size(item.size_bytes),
            item.kind.label().to_string(),
            item.risk.label().to_string(),
            item.reason.clone(),
        ]);
    }

    println!("\n{}", format!("[ Plan: {} ]", plan.name).cyan().bold());
    println!("  Source: {}", plan.source);
    println!("  Items: {}", plan.items.len());
    println!("  Total: {}", format_size(plan.total_bytes()).bold());
    println!("{}", table);
}

pub fn print_ok(msg: &str) {
    println!("  {} {}", "+".green(), msg);
}

pub fn print_warn(msg: &str) {
    println!("  {} {}", "!".yellow(), msg);
}

pub fn print_err(msg: &str) {
    println!("  {} {}", "x".red(), msg);
}

#[allow(dead_code)]
pub fn separator(label: &str) {
    let line = "-".repeat(60);
    println!("\n{}", format!("-- {} {}", label, line).dimmed());
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn format_size_bytes() {
        assert_eq!(format_size(0), "0.0 B");
        assert_eq!(format_size(512), "512.0 B");
    }

    #[test]
    fn format_size_kilobytes() {
        assert_eq!(format_size(1024), "1.0 KB");
    }

    #[test]
    fn format_size_megabytes() {
        assert_eq!(format_size(1024 * 1024), "1.0 MB");
    }

    #[test]
    fn format_size_gigabytes() {
        assert_eq!(format_size(1024 * 1024 * 1024), "1.0 GB");
    }
}
