use anyhow::{bail, Result};
use serde::{Deserialize, Serialize};
use std::fs;
use std::fs::Metadata;
#[cfg(unix)]
use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use walkdir::WalkDir;

use crate::core::fs::dir_size;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ScanMode {
    Fast,
    Deep,
}

impl ScanMode {
    pub fn budget(self) -> Option<Duration> {
        match self {
            ScanMode::Fast => Some(Duration::from_secs(8)),
            ScanMode::Deep => None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StorageScan {
    pub root: PathBuf,
    pub mode: ScanMode,
    pub depth: usize,
    pub limit: usize,
    pub scanned_at: u64,
    pub elapsed_ms: u128,
    pub partial: bool,
    pub tree: StorageNode,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StorageNode {
    pub name: String,
    pub path: PathBuf,
    pub size_bytes: u64,
    pub percent_of_parent: f64,
    pub is_dir: bool,
    pub partial: bool,
    pub children: Vec<StorageNode>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StorageCategory {
    pub name: String,
    pub size_bytes: u64,
    pub percent_of_total: f64,
    pub paths: Vec<PathBuf>,
    pub why: String,
    pub clean_with: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemDataScan {
    pub scanned_at: u64,
    pub elapsed_ms: u128,
    pub total_bytes: u64,
    pub partial: bool,
    pub categories: Vec<StorageCategory>,
    pub note: String,
}

#[derive(Clone)]
struct CategoryDef {
    name: &'static str,
    paths: Vec<PathBuf>,
    why: &'static str,
    clean_with: &'static str,
}

pub fn scan_tree(root: PathBuf, depth: usize, limit: usize, mode: ScanMode) -> Result<StorageScan> {
    if !root.exists() {
        bail!("Path does not exist: {}", root.display());
    }
    let start = Instant::now();
    let deadline = mode.budget().map(|budget| start + budget);
    let mut tree = build_node(&root, depth, limit, deadline, None)?;
    set_child_percentages(&mut tree);
    Ok(StorageScan {
        root,
        mode,
        depth,
        limit,
        scanned_at: now_secs(),
        elapsed_ms: start.elapsed().as_millis(),
        partial: tree.partial,
        tree,
    })
}

pub fn scan_system_data(mode: ScanMode) -> SystemDataScan {
    let start = Instant::now();
    let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
    let mut partial = false;
    let definitions = category_defs(&home);
    let per_category_budget = mode
        .budget()
        .map(|budget| budget / definitions.len().max(1) as u32);
    let mut categories: Vec<_> = definitions
        .into_iter()
        .map(|category| {
            let deadline = per_category_budget.map(|budget| Instant::now() + budget);
            let (size, category_partial) = match category.name {
                "Downloads Installers" => installers_size(&category.paths, deadline),
                "Containers" => containers_size(&home, &category.paths, deadline),
                _ => paths_size(&category.paths, deadline),
            };
            partial |= category_partial;
            StorageCategory {
                name: category.name.into(),
                size_bytes: size,
                percent_of_total: 0.0,
                paths: category.paths,
                why: category.why.into(),
                clean_with: category.clean_with.into(),
            }
        })
        .collect();

    categories.sort_by_key(|category| std::cmp::Reverse(category.size_bytes));
    let total_bytes: u64 = categories.iter().map(|category| category.size_bytes).sum();
    for category in &mut categories {
        category.percent_of_total = percent(category.size_bytes, total_bytes);
    }

    SystemDataScan {
        scanned_at: now_secs(),
        elapsed_ms: start.elapsed().as_millis(),
        total_bytes,
        partial,
        categories,
        note: "Explainable estimate from known macOS storage buckets, not Apple's private System Data number.".into(),
    }
}

pub fn write_tree_cache(scan: &StorageScan) -> Result<PathBuf> {
    let path = cache_dir()?.join("latest-tree.json");
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(&path, serde_json::to_string_pretty(scan)?)?;
    Ok(path)
}

pub fn write_system_data_cache(scan: &SystemDataScan) -> Result<PathBuf> {
    let path = cache_dir()?.join("latest-system-data.json");
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(&path, serde_json::to_string_pretty(scan)?)?;
    Ok(path)
}

fn cache_dir() -> Result<PathBuf> {
    let Some(home) = dirs::home_dir() else {
        bail!("Could not find home directory.");
    };
    Ok(home.join("Library/Application Support/macclean/scans"))
}

fn build_node(
    path: &Path,
    depth: usize,
    limit: usize,
    deadline: Option<Instant>,
    known_size: Option<u64>,
) -> Result<StorageNode> {
    let (size_bytes, mut partial) = match known_size {
        Some(size) => (size, false),
        None => bounded_path_size(path, deadline),
    };
    let mut node = StorageNode {
        name: path
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| path.display().to_string()),
        path: path.to_path_buf(),
        size_bytes,
        percent_of_parent: 100.0,
        is_dir: path.is_dir(),
        partial,
        children: Vec::new(),
    };

    if depth == 0 || !node.is_dir {
        return Ok(node);
    }

    let mut children = Vec::new();
    if let Ok(read_dir) = std::fs::read_dir(path) {
        for child in read_dir.filter_map(|e| e.ok()) {
            if deadline.is_some_and(|deadline| Instant::now() >= deadline) {
                node.partial = true;
                break;
            }

            let child_path = child.path();
            let (child_size, child_partial) = bounded_path_size(&child_path, deadline);
            partial |= child_partial;
            if child_size > 0 {
                children.push((child_path, child_size, child_partial));
            }
        }
    }

    children.sort_by_key(|(_, size, _)| std::cmp::Reverse(*size));
    children.truncate(limit);

    node.children = children
        .into_iter()
        .filter_map(|(child_path, child_size, child_partial)| {
            let mut child = build_node(
                &child_path,
                depth.saturating_sub(1),
                limit,
                deadline,
                Some(child_size),
            )
            .ok()?;
            child.partial |= child_partial;
            Some(child)
        })
        .collect();
    node.partial |= partial || node.children.iter().any(|child| child.partial);
    Ok(node)
}

fn set_child_percentages(node: &mut StorageNode) {
    for child in &mut node.children {
        child.percent_of_parent = percent(child.size_bytes, node.size_bytes);
        set_child_percentages(child);
    }
}

fn paths_size(paths: &[PathBuf], deadline: Option<Instant>) -> (u64, bool) {
    let mut total = 0u64;
    let mut partial = false;
    for path in paths.iter().filter(|path| path.exists()) {
        let (size, path_partial) = bounded_path_size(path, deadline);
        total = total.saturating_add(size);
        partial |= path_partial;
        if deadline.is_some_and(|deadline| Instant::now() >= deadline) {
            partial = true;
            break;
        }
    }
    (total, partial)
}

fn containers_size(home: &Path, paths: &[PathBuf], deadline: Option<Instant>) -> (u64, bool) {
    let (total, mut partial) = paths_size(paths, deadline);
    let docker_container = home.join("Library/Containers/com.docker.docker");
    if docker_container.exists() {
        let (docker_size, docker_partial) = bounded_path_size(&docker_container, deadline);
        partial |= docker_partial;
        (total.saturating_sub(docker_size), partial)
    } else {
        (total, partial)
    }
}

fn installers_size(paths: &[PathBuf], deadline: Option<Instant>) -> (u64, bool) {
    let mut total = 0u64;
    let mut partial = false;
    for path in paths.iter().filter(|path| path.exists()) {
        if deadline.is_some_and(|deadline| Instant::now() >= deadline) {
            partial = true;
            break;
        }
        if let Ok(read_dir) = std::fs::read_dir(path) {
            for entry in read_dir.filter_map(|e| e.ok()) {
                if deadline.is_some_and(|deadline| Instant::now() >= deadline) {
                    partial = true;
                    break;
                }
                let entry_path = entry.path();
                if entry_path.is_file() && is_installer(&entry_path) {
                    total = total.saturating_add(
                        entry_path
                            .metadata()
                            .map(|meta| allocated_size(&meta))
                            .unwrap_or(0),
                    );
                }
            }
        }
    }
    (total, partial)
}

fn bounded_path_size(path: &Path, deadline: Option<Instant>) -> (u64, bool) {
    if deadline.is_none() {
        if path.is_dir() {
            return (dir_size(path), false);
        }
        let size = path.metadata().map(|m| allocated_size(&m)).unwrap_or(0);
        return (size, false);
    }

    let Some(deadline) = deadline else {
        unreachable!();
    };
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

fn category_defs(home: &Path) -> Vec<CategoryDef> {
    vec![
        CategoryDef {
            name: "Xcode",
            paths: vec![
                home.join("Library/Developer/Xcode"),
                home.join("Library/Developer/CoreSimulator"),
                home.join("Library/Developer/CoreDevice"),
            ],
            why: "DerivedData, simulators, device support, archives.",
            clean_with: "macclean xcode",
        },
        CategoryDef {
            name: "iOS Backups",
            paths: vec![home.join("Library/Application Support/MobileSync/Backup")],
            why: "Local iPhone/iPad backup copies.",
            clean_with: "macclean ios-backups",
        },
        CategoryDef {
            name: "Docker",
            paths: vec![
                home.join("Library/Containers/com.docker.docker"),
                home.join(".docker"),
            ],
            why: "Images, volumes, containers, build cache.",
            clean_with: "macclean docker",
        },
        CategoryDef {
            name: "Application Support",
            paths: vec![home.join("Library/Application Support")],
            why: "App databases, media, indexes, local state.",
            clean_with: "macclean system-data --path",
        },
        CategoryDef {
            name: "Containers",
            paths: vec![
                home.join("Library/Containers"),
                home.join("Library/Group Containers"),
            ],
            why: "Sandboxed app data and group containers.",
            clean_with: "review; uninstall unused apps",
        },
        CategoryDef {
            name: "Caches",
            paths: vec![
                home.join("Library/Caches"),
                PathBuf::from("/Library/Caches"),
            ],
            why: "Regenerable app and system cache files.",
            clean_with: "macclean system/browser",
        },
        CategoryDef {
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
        CategoryDef {
            name: "Android",
            paths: vec![home.join("Library/Android"), home.join(".android")],
            why: "SDK caches, emulator data, Gradle/Android tools.",
            clean_with: "macclean android",
        },
        CategoryDef {
            name: "Logs & Reports",
            paths: vec![
                home.join("Library/Logs"),
                home.join("Library/Logs/DiagnosticReports"),
                PathBuf::from("/Library/Logs/DiagnosticReports"),
            ],
            why: "Logs, crash reports, diagnostics.",
            clean_with: "macclean crash-reports",
        },
        CategoryDef {
            name: "Downloads Installers",
            paths: vec![home.join("Downloads"), home.join("Desktop")],
            why: "DMG/PKG/ZIP installers often remain after install.",
            clean_with: "macclean installers",
        },
    ]
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

fn percent(value: u64, total: u64) -> f64 {
    if total == 0 {
        0.0
    } else {
        ((value as f64 / total as f64) * 1000.0).round() / 10.0
    }
}

fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn scan_tree_includes_percentages() {
        let dir = tempdir().unwrap();
        fs::create_dir(dir.path().join("a")).unwrap();
        fs::write(dir.path().join("a/file.txt"), vec![0u8; 1024]).unwrap();
        let scan = scan_tree(dir.path().to_path_buf(), 1, 10, ScanMode::Deep).unwrap();
        assert_eq!(scan.tree.children.len(), 1);
        assert!(scan.tree.children[0].percent_of_parent > 0.0);
    }

    #[test]
    fn percent_rounds_to_one_decimal() {
        assert_eq!(percent(1, 3), 33.3);
    }
}
