use crate::core::cmd::run_cmd;
use crate::core::storage::{self, ScanMode, StorageNode};
use crate::ui::{self, format_size};
use anyhow::Result;
use colored::Colorize;
use comfy_table::{presets::UTF8_BORDERS_ONLY, Table};
use std::path::PathBuf;

pub fn run(
    path: Option<PathBuf>,
    depth: usize,
    limit: usize,
    mode: ScanMode,
    json: bool,
) -> Result<()> {
    if let Some(path) = path {
        let scan = storage::scan_tree(path, depth, limit, mode)?;
        let _ = storage::write_tree_cache(&scan);
        if json {
            println!("{}", serde_json::to_string_pretty(&scan)?);
        } else {
            print_tree_scan(&scan);
        }
        return Ok(());
    }

    let scan = storage::scan_system_data(mode);
    let _ = storage::write_system_data_cache(&scan);
    if json {
        println!("{}", serde_json::to_string_pretty(&scan)?);
    } else {
        print_system_data_estimate(&scan);
    }
    Ok(())
}

fn print_system_data_estimate(scan: &storage::SystemDataScan) {
    println!("\n{}", "[ System Data Estimate ]".cyan().bold());
    println!("{}", scan.note.dimmed());
    if scan.partial {
        ui::print_warn("Fast scan hit its time budget; some category sizes are partial.");
    }

    let max = scan
        .categories
        .first()
        .map(|category| category.size_bytes)
        .unwrap_or(0);

    let mut table = Table::new();
    table.load_preset(UTF8_BORDERS_ONLY);
    table.set_header(vec![
        "Category",
        "Size",
        "Share",
        "Safety",
        "Map",
        "Why It Counts",
        "Action",
    ]);

    for category in scan
        .categories
        .iter()
        .filter(|category| category.size_bytes > 0)
    {
        table.add_row(vec![
            category.name.clone(),
            format_size(category.size_bytes),
            format!("{:.1}%", category.percent_of_total),
            category.safety.label().to_string(),
            bar(category.size_bytes, max, 18),
            category.why.clone(),
            category.clean_with.clone(),
        ]);
    }

    println!("{}", table);
    println!(
        "  Known buckets total: {}",
        format_size(scan.total_bytes).bold()
    );
    println!("  Scan time: {} ms", scan.elapsed_ms);
    print_snapshot_note();
}

fn print_snapshot_note() {
    let snapshots = run_cmd(&["tmutil", "listlocalsnapshots", "/"]);
    if snapshots.success() {
        let count = snapshots
            .output
            .lines()
            .filter(|line| line.trim().starts_with("com.apple.TimeMachine"))
            .count();
        if count > 0 {
            println!(
                "  {} {} local Time Machine snapshot(s) may also appear as System Data. Use `macclean timemachine` to review.",
                "!".yellow(),
                count
            );
        }
    }
}

fn print_tree_scan(scan: &storage::StorageScan) {
    println!(
        "\n{}",
        format!("[ Storage Tree: {} ]", scan.root.display())
            .cyan()
            .bold()
    );
    println!("{} {}", tree_prefix(false, true), root_label(&scan.tree));
    if scan.partial {
        ui::print_warn(
            "Fast tree scan hit its time budget; sizes shown are partial. Use --deep or a narrower --path for exact results.",
        );
    }
    for (i, child) in scan.tree.children.iter().enumerate() {
        render_tree(child, "", i + 1 == scan.tree.children.len());
    }
    println!("  Scan time: {} ms", scan.elapsed_ms);
}

fn render_tree(entry: &StorageNode, prefix: &str, last: bool) {
    println!(
        "{}{} {}",
        prefix,
        tree_prefix(last, false),
        entry_label(entry)
    );

    let child_prefix = format!("{}{}", prefix, if last { "    " } else { "│   " });
    for (i, child) in entry.children.iter().enumerate() {
        render_tree(child, &child_prefix, i + 1 == entry.children.len());
    }
}

fn root_label(entry: &StorageNode) -> String {
    format!(
        "{} {}",
        entry.name.bold(),
        format_size(entry.size_bytes).bold()
    )
}

fn entry_label(entry: &StorageNode) -> String {
    let partial = if entry.partial { " partial" } else { "" };
    format!(
        "{}  {}  {:>5.1}%  {:<9}  {}{}",
        entry.name,
        format_size(entry.size_bytes),
        entry.percent_of_parent,
        entry.safety.label(),
        percent_bar(entry.percent_of_parent, 12),
        partial.yellow()
    )
}

fn tree_prefix(last: bool, root: bool) -> &'static str {
    if root {
        "●"
    } else if last {
        "└──"
    } else {
        "├──"
    }
}

fn bar(size: u64, max: u64, width: usize) -> String {
    if max == 0 || width == 0 {
        return String::new();
    }
    let filled = ((size as f64 / max as f64) * width as f64).round() as usize;
    let filled = filled.clamp(1, width);
    format!("{}{}", "█".repeat(filled), "░".repeat(width - filled))
}

fn percent_bar(percent: f64, width: usize) -> String {
    if width == 0 {
        return String::new();
    }
    if percent <= 0.0 {
        return "░".repeat(width);
    }
    let filled = ((percent / 100.0) * width as f64).round() as usize;
    let filled = filled.clamp(1, width);
    format!("{}{}", "█".repeat(filled), "░".repeat(width - filled))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bar_has_requested_width() {
        assert_eq!(bar(50, 100, 10).chars().count(), 10);
        assert_eq!(percent_bar(25.0, 10).chars().count(), 10);
    }

    #[test]
    fn tree_prefixes_are_stable() {
        assert_eq!(tree_prefix(false, true), "●");
        assert_eq!(tree_prefix(false, false), "├──");
        assert_eq!(tree_prefix(true, false), "└──");
    }
}
