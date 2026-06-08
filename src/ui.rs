use colored::Colorize;
use comfy_table::{Table, presets::UTF8_BORDERS_ONLY};

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
    table.set_header(vec!["Location", "Size"]);
    for item in items {
        table.add_row(vec![item.label.clone(), format_size(item.size_bytes)]);
    }
    let total: u64 = items.iter().filter(|i| i.removable).map(|i| i.size_bytes).sum();
    println!();
    println!("{}", format!("[ {} ]", title).cyan().bold());
    println!("{}", table);
    println!("  Total recoverable: {}", format_size(total).bold());
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
