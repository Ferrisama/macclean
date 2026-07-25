use crate::core::cmd::run_cmd;
use crate::core::fs::dir_size;
use crate::ui::{self, format_size};
use anyhow::Result;
use colored::Colorize;
use comfy_table::{presets::UTF8_BORDERS_ONLY, Table};
use rayon::prelude::*;
use std::fs::Metadata;
#[cfg(unix)]
use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};
use walkdir::WalkDir;

#[derive(Clone)]
struct Category {
    name: &'static str,
    paths: Vec<PathBuf>,
    why: &'static str,
    clean_with: &'static str,
}

#[derive(Clone)]
struct CategorySize {
    name: &'static str,
    size: u64,
    why: &'static str,
    clean_with: &'static str,
}

#[derive(Clone)]
struct TreeEntry {
    name: String,
    size: u64,
    is_dir: bool,
    partial: bool,
    children: Vec<TreeEntry>,
}

pub fn run(path: Option<PathBuf>, depth: usize, limit: usize) -> Result<()> {
    if let Some(path) = path {
        print_tree(&path, depth, limit)?;
        return Ok(());
    }

    print_system_data_estimate();
    Ok(())
}

fn print_system_data_estimate() {
    let mut categories = collect_categories();
    categories.sort_by_key(|category| std::cmp::Reverse(category.size));

    println!("\n{}", "[ System Data Estimate ]".cyan().bold());
    println!(
        "{}",
        "This is an explainable estimate from known macOS storage buckets, not Apple's private System Data number."
            .dimmed()
    );

    let total: u64 = categories.iter().map(|category| category.size).sum();
    let max = categories
        .first()
        .map(|category| category.size)
        .unwrap_or(0);

    let mut table = Table::new();
    table.load_preset(UTF8_BORDERS_ONLY);
    table.set_header(vec!["Category", "Size", "Map", "Why It Counts", "Action"]);

    for category in &categories {
        if category.size == 0 {
            continue;
        }
        table.add_row(vec![
            category.name.to_string(),
            format_size(category.size),
            bar(category.size, max, 18),
            category.why.to_string(),
            category.clean_with.to_string(),
        ]);
    }

    println!("{}", table);
    println!("  Known buckets total: {}", format_size(total).bold());
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

fn collect_categories() -> Vec<CategorySize> {
    let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
    categories(&home)
        .into_iter()
        .par_bridge()
        .map(|category| {
            let size = match category.name {
                "Downloads Installers" => installers_size(&category.paths),
                "Containers" => containers_size(&home, &category.paths),
                _ => paths_size(&category.paths),
            };
            CategorySize {
                name: category.name,
                size,
                why: category.why,
                clean_with: category.clean_with,
            }
        })
        .collect()
}

fn categories(home: &Path) -> Vec<Category> {
    vec![
        Category {
            name: "Xcode",
            paths: vec![
                home.join("Library/Developer/Xcode"),
                home.join("Library/Developer/CoreSimulator"),
                home.join("Library/Developer/CoreDevice"),
            ],
            why: "DerivedData, simulators, device support, archives.",
            clean_with: "macclean xcode",
        },
        Category {
            name: "iOS Backups",
            paths: vec![home.join("Library/Application Support/MobileSync/Backup")],
            why: "Local iPhone/iPad backup copies.",
            clean_with: "macclean ios-backups",
        },
        Category {
            name: "Docker",
            paths: vec![
                home.join("Library/Containers/com.docker.docker"),
                home.join(".docker"),
            ],
            why: "Images, volumes, containers, build cache.",
            clean_with: "macclean docker",
        },
        Category {
            name: "Application Support",
            paths: vec![home.join("Library/Application Support")],
            why: "App databases, media, indexes, local state.",
            clean_with: "macclean system-data --path",
        },
        Category {
            name: "Containers",
            paths: vec![
                home.join("Library/Containers"),
                home.join("Library/Group Containers"),
            ],
            why: "Sandboxed app data and group containers.",
            clean_with: "review; uninstall unused apps",
        },
        Category {
            name: "Caches",
            paths: vec![
                home.join("Library/Caches"),
                PathBuf::from("/Library/Caches"),
            ],
            why: "Regenerable app and system cache files.",
            clean_with: "macclean system/browser",
        },
        Category {
            name: "Developer Caches",
            paths: vec![
                home.join(".gradle/caches"),
                home.join(".cargo/registry"),
                home.join(".npm"),
                home.join("Library/Caches/pip"),
                home.join("Library/Caches/go-build"),
            ],
            why: "Package manager and build caches.",
            clean_with: "macclean dev",
        },
        Category {
            name: "Android",
            paths: vec![home.join("Library/Android"), home.join(".android")],
            why: "SDK caches, emulator data, Gradle/Android tools.",
            clean_with: "macclean android",
        },
        Category {
            name: "Logs & Reports",
            paths: vec![
                home.join("Library/Logs"),
                home.join("Library/Logs/DiagnosticReports"),
                PathBuf::from("/Library/Logs/DiagnosticReports"),
            ],
            why: "Logs, crash reports, diagnostics.",
            clean_with: "macclean crash-reports",
        },
        Category {
            name: "Downloads Installers",
            paths: vec![home.join("Downloads"), home.join("Desktop")],
            why: "DMG/PKG/ZIP installers often remain after install.",
            clean_with: "macclean installers",
        },
    ]
}

fn paths_size(paths: &[PathBuf]) -> u64 {
    paths
        .par_iter()
        .filter(|path| path.exists())
        .map(|path| dir_size(path))
        .sum()
}

fn containers_size(home: &Path, paths: &[PathBuf]) -> u64 {
    let total = paths_size(paths);
    let docker_container = home.join("Library/Containers/com.docker.docker");
    if docker_container.exists() {
        total.saturating_sub(dir_size(&docker_container))
    } else {
        total
    }
}

fn installers_size(paths: &[PathBuf]) -> u64 {
    paths
        .par_iter()
        .filter(|path| path.exists())
        .map(|path| {
            std::fs::read_dir(path)
                .ok()
                .into_iter()
                .flat_map(|entries| entries.filter_map(|e| e.ok()))
                .map(|entry| entry.path())
                .filter(|path| path.is_file() && is_installer(path))
                .filter_map(|path| path.metadata().ok().map(|meta| meta.len()))
                .sum::<u64>()
        })
        .sum()
}

fn is_installer(path: &Path) -> bool {
    let name = path
        .file_name()
        .map(|n| n.to_string_lossy().to_lowercase())
        .unwrap_or_default();
    matches!(
        path.extension().and_then(|e| e.to_str()),
        Some("dmg") | Some("pkg") | Some("zip")
    ) || name.ends_with(".tar.gz")
        || name.ends_with(".tar.bz2")
}

fn print_tree(root: &Path, depth: usize, limit: usize) -> Result<()> {
    if !root.exists() {
        anyhow::bail!("Path does not exist: {}", root.display());
    }

    let tree = build_tree(root, depth, limit)?;
    println!(
        "\n{}",
        format!("[ Storage Tree: {} ]", root.display())
            .cyan()
            .bold()
    );
    println!("{} {}", tree_prefix(false, true), root_label(&tree));
    if tree.partial {
        ui::print_warn(
            "Fast tree scan hit the 8s budget; sizes shown are partial. Try a narrower path, e.g. ~/Library/Caches.",
        );
    }
    for (i, child) in tree.children.iter().enumerate() {
        render_tree(child, "", i + 1 == tree.children.len(), tree.size);
    }
    Ok(())
}

fn build_tree(path: &Path, depth: usize, limit: usize) -> Result<TreeEntry> {
    if depth <= 1 {
        return Ok(build_shallow_tree_bounded(
            path,
            limit,
            Duration::from_secs(8),
        ));
    }

    build_tree_with_size(path, depth, limit, None)
}

fn build_shallow_tree_bounded(path: &Path, limit: usize, budget: Duration) -> TreeEntry {
    let deadline = Instant::now() + budget;
    let mut root_size = 0u64;
    let mut children = Vec::new();
    let mut partial = false;

    if let Ok(read_dir) = std::fs::read_dir(path) {
        for child in read_dir.filter_map(|e| e.ok()) {
            if Instant::now() >= deadline {
                partial = true;
                break;
            }

            let entry_path = child.path();
            let (size, child_partial) = bounded_path_size(&entry_path, deadline);
            partial |= child_partial;
            root_size = root_size.saturating_add(size);
            if size > 0 {
                children.push(TreeEntry {
                    name: entry_path
                        .file_name()
                        .map(|n| n.to_string_lossy().to_string())
                        .unwrap_or_else(|| entry_path.display().to_string()),
                    size,
                    is_dir: entry_path.is_dir(),
                    partial: child_partial,
                    children: Vec::new(),
                });
            }
        }
    }

    children.sort_by_key(|entry| std::cmp::Reverse(entry.size));
    children.truncate(limit);

    TreeEntry {
        name: path
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| path.display().to_string()),
        size: root_size,
        is_dir: path.is_dir(),
        partial,
        children,
    }
}

fn bounded_path_size(path: &Path, deadline: Instant) -> (u64, bool) {
    if path.is_file() {
        let size = path.metadata().map(|m| allocated_size(&m)).unwrap_or(0);
        return (size, false);
    }

    let mut total = 0u64;
    let mut partial = false;
    for entry in WalkDir::new(path).follow_links(false).into_iter() {
        if Instant::now() >= deadline {
            partial = true;
            break;
        }

        let Ok(entry) = entry else {
            continue;
        };
        if entry.file_type().is_file() {
            total = total.saturating_add(entry.metadata().map(|m| allocated_size(&m)).unwrap_or(0));
        }
    }

    (total, partial)
}

#[cfg(unix)]
fn allocated_size(metadata: &Metadata) -> u64 {
    let allocated = metadata.blocks().saturating_mul(512);
    if allocated == 0 && metadata.len() > 0 {
        metadata.len()
    } else {
        allocated
    }
}

#[cfg(not(unix))]
fn allocated_size(metadata: &Metadata) -> u64 {
    metadata.len()
}

fn build_tree_with_size(
    path: &Path,
    depth: usize,
    limit: usize,
    known_size: Option<u64>,
) -> Result<TreeEntry> {
    let size = known_size.unwrap_or_else(|| {
        if path.is_dir() {
            dir_size(path)
        } else {
            path.metadata().map(|m| m.len()).unwrap_or(0)
        }
    });

    let mut entry = TreeEntry {
        name: path
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| path.display().to_string()),
        size,
        is_dir: path.is_dir(),
        partial: false,
        children: Vec::new(),
    };

    if depth == 0 || !entry.is_dir {
        return Ok(entry);
    }

    let mut children = Vec::new();
    if let Ok(read_dir) = std::fs::read_dir(path) {
        for child in read_dir.filter_map(|e| e.ok()) {
            let child_path = child.path();
            let child_size = if child_path.is_dir() {
                dir_size(&child_path)
            } else {
                child_path.metadata().map(|m| m.len()).unwrap_or(0)
            };
            if child_size == 0 {
                continue;
            }
            children.push((child_path, child_size));
        }
    }

    children.sort_by_key(|(_, size)| std::cmp::Reverse(*size));
    children.truncate(limit);

    entry.children = children
        .into_iter()
        .filter_map(|(path, size)| {
            build_tree_with_size(&path, depth.saturating_sub(1), limit, Some(size)).ok()
        })
        .collect();

    Ok(entry)
}

fn render_tree(entry: &TreeEntry, prefix: &str, last: bool, parent_size: u64) {
    println!(
        "{}{} {}",
        prefix,
        tree_prefix(last, false),
        entry_label(entry, parent_size)
    );

    let child_prefix = format!("{}{}", prefix, if last { "    " } else { "│   " });
    for (i, child) in entry.children.iter().enumerate() {
        render_tree(
            child,
            &child_prefix,
            i + 1 == entry.children.len(),
            entry.size,
        );
    }
}

fn root_label(entry: &TreeEntry) -> String {
    format!("{} {}", entry.name.bold(), format_size(entry.size).bold())
}

fn entry_label(entry: &TreeEntry, parent_size: u64) -> String {
    let pct = if parent_size > 0 {
        (entry.size as f64 / parent_size as f64) * 100.0
    } else {
        0.0
    };
    format!(
        "{}  {}  {:>5.1}%  {}",
        entry.name,
        format_size(entry.size),
        pct,
        bar(entry.size, parent_size, 12)
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bar_has_requested_width() {
        assert_eq!(bar(50, 100, 10).chars().count(), 10);
    }

    #[test]
    fn tree_prefixes_are_stable() {
        assert_eq!(tree_prefix(false, true), "●");
        assert_eq!(tree_prefix(false, false), "├──");
        assert_eq!(tree_prefix(true, false), "└──");
    }
}
