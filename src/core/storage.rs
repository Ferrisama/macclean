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
use crate::core::CleanKind;

const FAST_SCAN_BUDGET: Duration = Duration::from_secs(8);
type ScanProgressCallback<'a> = dyn Fn(&Path, u64, bool, usize, usize) + Sync + 'a;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ScanMode {
    Fast,
    Deep,
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
    #[serde(skip_serializing_if = "Option::is_none")]
    pub incomplete_reason: Option<String>,
    #[serde(default)]
    pub metrics: ScanMetrics,
    pub tree: StorageNode,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ScanMetrics {
    /// The scanner implementation used to produce this result. Keeping this in
    /// the result lets benchmark captures distinguish old cached data from the
    /// one-pass traversal.
    pub implementation: String,
    pub entries_seen: u64,
    pub directories_seen: u64,
    pub files_seen: u64,
    pub metadata_errors: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StorageNode {
    pub name: String,
    pub path: PathBuf,
    pub size_bytes: u64,
    pub percent_of_parent: f64,
    pub is_dir: bool,
    pub partial: bool,
    pub safety: StorageSafety,
    pub clean_kind: CleanKind,
    pub cleanup_action: String,
    pub cleanup_reason: String,
    pub children: Vec<StorageNode>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StorageCategory {
    pub name: String,
    pub size_bytes: u64,
    pub percent_of_total: f64,
    pub safety: StorageSafety,
    pub clean_kind: CleanKind,
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
    safety: StorageSafety,
    clean_kind: CleanKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum StorageSafety {
    Safe,
    Review,
    UserData,
    Protected,
    Unknown,
}

impl StorageSafety {
    pub fn label(self) -> &'static str {
        match self {
            StorageSafety::Safe => "safe",
            StorageSafety::Review => "review",
            StorageSafety::UserData => "user data",
            StorageSafety::Protected => "protected",
            StorageSafety::Unknown => "unknown",
        }
    }
}

pub fn scan_tree(root: PathBuf, depth: usize, limit: usize, mode: ScanMode) -> Result<StorageScan> {
    scan_tree_with_progress(root, depth, limit, mode, None)
}

pub fn scan_tree_with_progress(
    root: PathBuf,
    depth: usize,
    limit: usize,
    mode: ScanMode,
    progress: Option<&ScanProgressCallback<'_>>,
) -> Result<StorageScan> {
    if !root.exists() {
        bail!("Path does not exist: {}", root.display());
    }
    let start = Instant::now();
    let (mut tree, metrics) = if mode == ScanMode::Fast {
        let tree = build_fast_tree(
            &root,
            limit,
            progress,
            Some(Instant::now() + FAST_SCAN_BUDGET),
        )?;
        (
            tree,
            ScanMetrics {
                implementation: "bounded-overview".into(),
                ..ScanMetrics::default()
            },
        )
    } else {
        let mut metrics = ScanMetrics {
            implementation: "single-pass".into(),
            ..ScanMetrics::default()
        };
        let tree = build_single_pass_tree(&root, depth, limit, progress, &mut metrics)?;
        (tree, metrics)
    };
    set_child_percentages(&mut tree);
    let partial = tree.partial;
    Ok(StorageScan {
        root,
        mode,
        depth,
        limit,
        scanned_at: now_secs(),
        elapsed_ms: start.elapsed().as_millis(),
        partial,
        incomplete_reason: partial.then(|| {
            "One or more paths could not be fully read or sized; totals may be incomplete.".into()
        }),
        metrics,
        tree,
    })
}

pub fn scan_system_data(_mode: ScanMode) -> SystemDataScan {
    let start = Instant::now();
    let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
    let mut partial = false;
    let definitions = category_defs(&home);
    let mut categories: Vec<_> = definitions
        .into_iter()
        .map(|category| {
            let (size, category_partial) = match category.name {
                "Downloads Installers" => installers_size(&category.paths, None),
                "Containers" => containers_size(&home, &category.paths, None),
                _ => paths_size(&category.paths, None),
            };
            partial |= category_partial;
            StorageCategory {
                name: category.name.into(),
                size_bytes: size,
                percent_of_total: 0.0,
                safety: category.safety,
                clean_kind: category.clean_kind,
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

pub fn app_cleanup_allowed(path: &Path) -> bool {
    matches!(
        app_cleanup_safety(path),
        StorageSafety::Safe | StorageSafety::Review
    )
}

/// The safety tier used by the app's direct Trash operation. Recipes use this
/// exact classification so their selectable state matches execution.
pub fn app_cleanup_safety(path: &Path) -> StorageSafety {
    classify_storage_path(path, path.is_dir()).safety
}

fn cache_dir() -> Result<PathBuf> {
    let Some(home) = dirs::home_dir() else {
        bail!("Could not find home directory.");
    };
    Ok(home.join("Library/Application Support/macclean/scans"))
}

/// Traverse a selected scope exactly once. Every child contributes its
/// allocation to each ancestor as the walk unwinds; presentation depth only
/// controls which already-measured nodes we retain in the returned tree.
fn build_single_pass_tree(
    root: &Path,
    visible_depth: usize,
    limit: usize,
    progress: Option<&ScanProgressCallback<'_>>,
    metrics: &mut ScanMetrics,
) -> Result<StorageNode> {
    let root = root.canonicalize().unwrap_or_else(|_| root.to_path_buf());
    metrics.entries_seen = metrics.entries_seen.saturating_add(1);
    let metadata = match fs::symlink_metadata(&root) {
        Ok(metadata) => metadata,
        Err(_) => {
            metrics.metadata_errors = metrics.metadata_errors.saturating_add(1);
            return Ok(classified_node(&root, 0, false, true));
        }
    };
    if !metadata.file_type().is_dir() {
        metrics.files_seen = metrics.files_seen.saturating_add(1);
        return Ok(classified_node(
            &root,
            allocated_size(&metadata),
            false,
            false,
        ));
    }

    metrics.directories_seen = metrics.directories_seen.saturating_add(1);
    let mut root_node = classified_node(&root, 0, true, false);
    let entries: Vec<_> = match fs::read_dir(&root) {
        Ok(entries) => entries.collect(),
        Err(_) => {
            metrics.metadata_errors = metrics.metadata_errors.saturating_add(1);
            root_node.partial = true;
            return Ok(root_node);
        }
    };
    let total = entries.len();
    let mut retained_children = Vec::new();
    for (index, entry) in entries.into_iter().enumerate() {
        let entry = match entry {
            Ok(entry) => entry,
            Err(_) => {
                metrics.metadata_errors = metrics.metadata_errors.saturating_add(1);
                root_node.partial = true;
                continue;
            }
        };
        let child = single_pass_node(
            &entry.path(),
            visible_depth.saturating_sub(1),
            limit,
            metrics,
        )?;
        root_node.size_bytes = root_node.size_bytes.saturating_add(child.size_bytes);
        root_node.partial |= child.partial;
        if let Some(progress) = progress {
            progress(
                &child.path,
                child.size_bytes,
                child.partial,
                index + 1,
                total,
            );
        }
        if visible_depth > 0 && child.size_bytes > 0 {
            retained_children.push(child);
        }
    }
    retained_children.sort_by_key(|child| std::cmp::Reverse(child.size_bytes));
    retained_children.truncate(limit);
    root_node.children = retained_children;
    Ok(root_node)
}

fn single_pass_node(
    path: &Path,
    visible_depth: usize,
    limit: usize,
    metrics: &mut ScanMetrics,
) -> Result<StorageNode> {
    metrics.entries_seen = metrics.entries_seen.saturating_add(1);
    let metadata = match fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(_) => {
            metrics.metadata_errors = metrics.metadata_errors.saturating_add(1);
            return Ok(classified_node(path, 0, false, true));
        }
    };
    let is_dir = metadata.file_type().is_dir();
    if !is_dir {
        metrics.files_seen = metrics.files_seen.saturating_add(1);
        return Ok(classified_node(
            path,
            allocated_size(&metadata),
            false,
            false,
        ));
    }

    metrics.directories_seen = metrics.directories_seen.saturating_add(1);
    let mut node = classified_node(path, 0, true, false);
    let read_dir = match fs::read_dir(path) {
        Ok(entries) => entries,
        Err(_) => {
            metrics.metadata_errors = metrics.metadata_errors.saturating_add(1);
            node.partial = true;
            return Ok(node);
        }
    };

    let mut retained_children = Vec::new();
    for entry in read_dir {
        let entry = match entry {
            Ok(entry) => entry,
            Err(_) => {
                metrics.metadata_errors = metrics.metadata_errors.saturating_add(1);
                node.partial = true;
                continue;
            }
        };
        let child = single_pass_node(
            &entry.path(),
            visible_depth.saturating_sub(1),
            limit,
            metrics,
        )?;
        node.size_bytes = node.size_bytes.saturating_add(child.size_bytes);
        node.partial |= child.partial;
        if visible_depth > 0 && child.size_bytes > 0 {
            retained_children.push(child);
        }
    }
    retained_children.sort_by_key(|child| std::cmp::Reverse(child.size_bytes));
    retained_children.truncate(limit);
    node.children = retained_children;
    Ok(node)
}

// Kept temporarily for system-data helpers that still use bounded sizing.
// `scan_tree` no longer dispatches through this repeated-sizing path.
#[allow(dead_code, clippy::too_many_arguments)]
fn build_node(
    path: &Path,
    depth: usize,
    limit: usize,
    deadline: Option<Instant>,
    known_size: Option<u64>,
    progress: Option<&ScanProgressCallback<'_>>,
    completed: usize,
    total: usize,
) -> Result<StorageNode> {
    let is_dir = path.is_dir();
    let (size_bytes, mut partial) = if depth == 0 || !is_dir {
        match known_size {
            Some(size) => (size, false),
            None => bounded_path_size(path, deadline),
        }
    } else {
        (known_size.unwrap_or(0), false)
    };
    let mut node = StorageNode {
        name: path
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| path.display().to_string()),
        path: path.to_path_buf(),
        size_bytes,
        percent_of_parent: 100.0,
        is_dir,
        partial,
        safety: StorageSafety::Unknown,
        clean_kind: CleanKind::Unknown,
        cleanup_action: String::new(),
        cleanup_reason: String::new(),
        children: Vec::new(),
    };
    let classification = classify_storage_path(path, node.is_dir);
    node.safety = classification.safety;
    node.clean_kind = classification.clean_kind;
    node.cleanup_action = classification.action;
    node.cleanup_reason = classification.reason;

    if depth == 0 || !node.is_dir {
        return Ok(node);
    }

    let child_paths = std::fs::read_dir(path)
        .map(|read_dir| {
            read_dir
                .filter_map(|entry| entry.ok().map(|entry| entry.path()))
                .collect()
        })
        .unwrap_or_else(|_| {
            node.partial = true;
            Vec::new()
        });

    let mut children = Vec::new();
    for (index, child_path) in child_paths.iter().enumerate() {
        if deadline.is_some_and(|deadline| Instant::now() >= deadline) {
            node.partial = true;
            break;
        }

        let remaining_children = child_paths.len().saturating_sub(index).max(1);
        let child_deadline = per_child_deadline(deadline, remaining_children);
        let (child_size, child_partial) = bounded_path_size(child_path, child_deadline);
        partial |= child_partial;
        if let Some(progress) = progress {
            progress(
                child_path,
                child_size,
                child_partial,
                completed + index + 1,
                total.max(child_paths.len()),
            );
        }
        if child_size > 0 {
            children.push((child_path.clone(), child_size, child_partial));
        }
    }

    children.sort_by_key(|(_, size, _)| std::cmp::Reverse(*size));
    let measured_size = children
        .iter()
        .fold(0u64, |total, (_, size, _)| total.saturating_add(*size));
    if known_size.is_none() {
        node.size_bytes = measured_size;
    }
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
                progress,
                completed + 1,
                total.max(child_paths.len()),
            )
            .ok()?;
            child.partial |= child_partial;
            Some(child)
        })
        .collect();
    node.partial |= partial || node.children.iter().any(|child| child.partial);
    Ok(node)
}

fn build_fast_tree(
    root: &Path,
    limit: usize,
    progress: Option<&ScanProgressCallback<'_>>,
    deadline: Option<Instant>,
) -> Result<StorageNode> {
    let root = root.canonicalize().unwrap_or_else(|_| root.to_path_buf());
    if !root.is_dir() {
        return build_node(&root, 0, limit, deadline, None, progress, 0, 0);
    }

    let mut root_node = classified_node(&root, 0, true, false);
    let child_paths: Vec<PathBuf> = fs::read_dir(&root)
        .map(|read_dir| {
            read_dir
                .filter_map(|entry| entry.ok().map(|entry| entry.path()))
                .collect()
        })
        .unwrap_or_else(|_| {
            root_node.partial = true;
            Vec::new()
        });

    let total = child_paths.len();
    let mut children: Vec<_> = child_paths
        .iter()
        .enumerate()
        .filter_map(|(index, child)| {
            let is_dir = child.is_dir();
            let child_deadline = per_child_deadline(deadline, total.saturating_sub(index));
            let (size, partial) = bounded_path_size(child, child_deadline);
            if let Some(progress) = progress {
                progress(child, size, partial, index + 1, total);
            }
            (size > 0).then(|| classified_node(child, size, is_dir, partial))
        })
        .collect();

    children.sort_by_key(|child| std::cmp::Reverse(child.size_bytes));
    root_node.partial |= children.iter().any(|child| child.partial);
    root_node.size_bytes = children
        .iter()
        .fold(0u64, |total, child| total.saturating_add(child.size_bytes));
    children.truncate(limit);
    root_node.children = children;
    Ok(root_node)
}

pub(crate) fn classified_node(
    path: &Path,
    size_bytes: u64,
    is_dir: bool,
    partial: bool,
) -> StorageNode {
    let classification = classify_storage_path(path, is_dir);
    StorageNode {
        name: path
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| path.display().to_string()),
        path: path.to_path_buf(),
        size_bytes,
        percent_of_parent: 100.0,
        is_dir,
        partial,
        safety: classification.safety,
        clean_kind: classification.clean_kind,
        cleanup_action: classification.action,
        cleanup_reason: classification.reason,
        children: Vec::new(),
    }
}

#[allow(dead_code)]
fn per_child_deadline(deadline: Option<Instant>, remaining_children: usize) -> Option<Instant> {
    let deadline = deadline?;
    let now = Instant::now();
    if now >= deadline {
        return Some(now);
    }
    let remaining = deadline.saturating_duration_since(now);
    Some(now + (remaining / remaining_children.max(1) as u32))
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
            safety: StorageSafety::Review,
            clean_kind: CleanKind::DevArtifact,
        },
        CategoryDef {
            name: "iOS Backups",
            paths: vec![home.join("Library/Application Support/MobileSync/Backup")],
            why: "Local iPhone/iPad backup copies.",
            clean_with: "macclean ios-backups",
            safety: StorageSafety::Review,
            clean_kind: CleanKind::Backup,
        },
        CategoryDef {
            name: "Docker",
            paths: vec![
                home.join("Library/Containers/com.docker.docker"),
                home.join(".docker"),
            ],
            why: "Images, volumes, containers, build cache.",
            clean_with: "macclean docker",
            safety: StorageSafety::Review,
            clean_kind: CleanKind::DevArtifact,
        },
        CategoryDef {
            name: "Application Support",
            paths: vec![home.join("Library/Application Support")],
            why: "App databases, media, indexes, local state.",
            clean_with: "macclean system-data --path",
            safety: StorageSafety::UserData,
            clean_kind: CleanKind::Unknown,
        },
        CategoryDef {
            name: "Containers",
            paths: vec![
                home.join("Library/Containers"),
                home.join("Library/Group Containers"),
            ],
            why: "Sandboxed app data and group containers.",
            clean_with: "review; uninstall unused apps",
            safety: StorageSafety::Review,
            clean_kind: CleanKind::AppTrace,
        },
        CategoryDef {
            name: "Caches",
            paths: vec![
                home.join("Library/Caches"),
                PathBuf::from("/Library/Caches"),
            ],
            why: "Regenerable app and system cache files.",
            clean_with: "macclean system/browser",
            safety: StorageSafety::Safe,
            clean_kind: CleanKind::Cache,
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
            safety: StorageSafety::Safe,
            clean_kind: CleanKind::DevArtifact,
        },
        CategoryDef {
            name: "Android",
            paths: vec![home.join("Library/Android"), home.join(".android")],
            why: "SDK caches, emulator data, Gradle/Android tools.",
            clean_with: "macclean android",
            safety: StorageSafety::Review,
            clean_kind: CleanKind::DevArtifact,
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
            safety: StorageSafety::Safe,
            clean_kind: CleanKind::Log,
        },
        CategoryDef {
            name: "Downloads Installers",
            paths: vec![home.join("Downloads"), home.join("Desktop")],
            why: "DMG/PKG/ZIP installers often remain after install.",
            clean_with: "macclean installers",
            safety: StorageSafety::Review,
            clean_kind: CleanKind::Installer,
        },
    ]
}

struct StorageClassification {
    safety: StorageSafety,
    clean_kind: CleanKind,
    action: String,
    reason: String,
}

fn classify_storage_path(path: &Path, is_dir: bool) -> StorageClassification {
    let lower_path = path.to_string_lossy().to_lowercase();
    let components = lowercase_components(path);
    let lower_name = path
        .file_name()
        .map(|name| name.to_string_lossy().to_lowercase())
        .unwrap_or_default();

    let classified = if lower_path.starts_with("/system/")
        || lower_path.starts_with("/private/var/db/")
        || lower_path.starts_with("/usr/")
        || lower_path.contains("/library/keychains")
    {
        (
            StorageSafety::Protected,
            CleanKind::Unknown,
            "Do not clean automatically.",
            "System-owned or security-sensitive location.",
        )
    } else if has_component_sequence(&components, &["library", "caches"])
        || has_component_sequence(&components, &[".npm"])
        || has_component_sequence(&components, &[".yarn", "cache"])
        || has_component_sequence(&components, &[".yarn", "berry", "cache"])
        || has_component_sequence(&components, &[".local", "share", "pnpm", "store"])
        || has_component_sequence(&components, &["library", "pnpm", "store"])
        || has_component_sequence(&components, &[".cargo", "registry", "cache"])
        || has_component_sequence(&components, &[".cargo", "registry", "src"])
        || has_component_sequence(&components, &[".cargo", "git", "checkouts"])
        || has_component_sequence(
            &components,
            &[
                "library",
                "application support",
                "stremio-server",
                "stremio-cache",
            ],
        )
        || has_component_sequence(
            &components,
            &[
                "library",
                "application support",
                "stremio-server",
                "server-cache",
            ],
        )
        || has_component_sequence(
            &components,
            &[
                "library",
                "application support",
                "smart code ltd",
                "stremio",
                "cache",
            ],
        )
        || has_component_sequence(
            &components,
            &[
                "library",
                "application support",
                "smart code ltd",
                "stremio",
                "code cache",
            ],
        )
        || has_component_sequence(
            &components,
            &[
                "library",
                "application support",
                "smart code ltd",
                "stremio",
                "gpucache",
            ],
        )
        || has_component_sequence(
            &components,
            &[
                "library",
                "application support",
                "smart code ltd",
                "stremio",
                "dawncache",
            ],
        )
        || has_component_sequence(
            &components,
            &[
                "library",
                "application support",
                "smart code ltd",
                "stremio",
                "service worker",
                "cachestorage",
            ],
        )
        || has_component_sequence(
            &components,
            &[
                "library",
                "application support",
                "smart code ltd",
                "stremio",
                "blob_storage",
            ],
        )
    {
        (
            StorageSafety::Safe,
            CleanKind::Cache,
            "Review in Clean, then remove cache contents.",
            "Regenerable cache data.",
        )
    } else if has_component_sequence(&components, &["library", "logs"])
        || components
            .iter()
            .any(|component| component == "diagnosticreports")
        || lower_name.ends_with(".log")
    {
        (
            StorageSafety::Safe,
            CleanKind::Log,
            "Review in Clean, then remove logs or reports.",
            "Logs and diagnostic reports are usually safe to remove.",
        )
    } else if has_component_sequence(
        &components,
        &["library", "developer", "xcode", "deriveddata"],
    ) || components
        .iter()
        .any(|component| component == "node_modules")
        || has_component_sequence(&components, &[".gradle", "caches"])
        || has_component_sequence(&components, &[".npm", "_cacache"])
    {
        (
            StorageSafety::Safe,
            CleanKind::DevArtifact,
            "Review in Developer cleanup.",
            "Generated developer artifact that can usually be rebuilt.",
        )
    } else if lower_name == ".ollama"
        || lower_name == ".rustup"
        || lower_name == ".nvm"
        || lower_name == ".pyenv"
        || has_component_sequence(&components, &[".rustup", "toolchains"])
        || has_component_sequence(&components, &[".nvm", "versions"])
    {
        (
            StorageSafety::Review,
            CleanKind::DevArtifact,
            "Review installed models, runtimes, or toolchains before removing.",
            "Developer/runtime data can be large but may be actively used.",
        )
    } else if components
        .iter()
        .any(|component| component == "com.docker.docker")
        || components
            .iter()
            .any(|component| component == "coresimulator")
        || has_component_sequence(&components, &["mobilesync", "backup"])
        || lower_name == ".venv"
        || lower_name == "venv"
    {
        (
            StorageSafety::Review,
            CleanKind::DevArtifact,
            "Open a review plan before cleaning.",
            "Often removable, but may contain workflow state or backups.",
        )
    } else if is_installer(path) {
        (
            StorageSafety::Review,
            CleanKind::Installer,
            "Review installer before moving to Trash.",
            "Installers are often expendable after installation.",
        )
    } else if lower_path.contains("/desktop")
        || lower_path.contains("/documents")
        || lower_path.contains("/downloads")
        || lower_path.contains("/movies")
        || lower_path.contains("/music")
        || lower_path.contains("/pictures")
        || lower_path.contains("/photos library")
        || lower_path.contains("/library/application support")
    {
        (
            StorageSafety::UserData,
            CleanKind::Unknown,
            "Reveal in Finder; do not auto-select.",
            "May contain original user or application data.",
        )
    } else if is_dir {
        (
            StorageSafety::Unknown,
            CleanKind::Unknown,
            "Inspect before taking action.",
            "No known cleanup rule matched this folder.",
        )
    } else {
        (
            StorageSafety::Unknown,
            CleanKind::Unknown,
            "Inspect before taking action.",
            "No known cleanup rule matched this file.",
        )
    };

    StorageClassification {
        safety: classified.0,
        clean_kind: classified.1,
        action: classified.2.to_string(),
        reason: classified.3.to_string(),
    }
}

fn lowercase_components(path: &Path) -> Vec<String> {
    path.components()
        .filter_map(|component| match component {
            std::path::Component::Normal(name) => Some(name.to_string_lossy().to_lowercase()),
            _ => None,
        })
        .collect()
}

fn has_component_sequence(components: &[String], expected: &[&str]) -> bool {
    components.windows(expected.len()).any(|window| {
        window
            .iter()
            .map(String::as_str)
            .eq(expected.iter().copied())
    })
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
    fn single_pass_scan_conserves_bytes_and_reports_coverage() {
        let dir = tempdir().unwrap();
        fs::create_dir_all(dir.path().join("alpha/nested")).unwrap();
        fs::create_dir(dir.path().join("beta")).unwrap();
        fs::write(dir.path().join("alpha/one.bin"), vec![1u8; 128]).unwrap();
        fs::write(dir.path().join("alpha/nested/two.bin"), vec![2u8; 256]).unwrap();
        fs::write(dir.path().join("beta/three.bin"), vec![3u8; 512]).unwrap();

        let scan = scan_tree(dir.path().to_path_buf(), 3, 10, ScanMode::Deep).unwrap();
        let child_total = scan
            .tree
            .children
            .iter()
            .fold(0u64, |total, child| total.saturating_add(child.size_bytes));

        assert_eq!(scan.metrics.implementation, "single-pass");
        assert_eq!(scan.metrics.metadata_errors, 0);
        assert_eq!(scan.metrics.directories_seen, 4);
        assert_eq!(scan.metrics.files_seen, 3);
        assert_eq!(scan.metrics.entries_seen, 7);
        assert_eq!(scan.tree.size_bytes, child_total);
        assert!(!scan.tree.partial);
    }

    #[test]
    fn single_pass_scan_streams_each_root_child_before_completion() {
        let dir = tempdir().unwrap();
        fs::create_dir_all(dir.path().join("large/nested")).unwrap();
        fs::write(dir.path().join("large/nested/file.bin"), vec![0u8; 512]).unwrap();
        fs::write(dir.path().join("small.bin"), vec![0u8; 64]).unwrap();
        let events = std::sync::Mutex::new(Vec::new());

        let _ = scan_tree_with_progress(
            dir.path().to_path_buf(),
            1,
            1,
            ScanMode::Fast,
            Some(&|path, _, _, scanned, estimated| {
                events.lock().unwrap().push((
                    path.file_name().unwrap().to_string_lossy().to_string(),
                    scanned,
                    estimated,
                ));
            }),
        )
        .unwrap();

        let events = events.into_inner().unwrap();
        assert_eq!(events.len(), 2);
        assert!(events.iter().all(|(_, _, estimated)| *estimated == 2));
        assert_eq!(events.last().unwrap().1, 2);
    }

    #[test]
    fn fast_scan_stays_top_level_while_deep_scan_expands_children() {
        let dir = tempdir().unwrap();
        let nested = dir.path().join("parent/child");
        fs::create_dir_all(&nested).unwrap();
        fs::write(nested.join("file.txt"), vec![0u8; 1024]).unwrap();

        let fast = scan_tree(dir.path().to_path_buf(), 4, 10, ScanMode::Fast).unwrap();
        let deep = scan_tree(dir.path().to_path_buf(), 4, 10, ScanMode::Deep).unwrap();

        assert_eq!(fast.mode, ScanMode::Fast);
        assert_eq!(deep.mode, ScanMode::Deep);
        assert!(fast
            .tree
            .children
            .iter()
            .all(|child| child.children.is_empty()));
        assert!(deep
            .tree
            .children
            .iter()
            .any(|child| !child.children.is_empty()));
    }

    #[test]
    fn scan_tree_classifies_cache_paths_as_safe() {
        let dir = tempdir().unwrap();
        let cache_dir = dir.path().join("Library/Caches/Foo");
        fs::create_dir_all(&cache_dir).unwrap();
        fs::write(cache_dir.join("cache.bin"), vec![0u8; 1024]).unwrap();

        let scan = scan_tree(dir.path().to_path_buf(), 3, 10, ScanMode::Deep).unwrap();
        let library = scan
            .tree
            .children
            .iter()
            .find(|child| child.name == "Library")
            .unwrap();
        let caches = library
            .children
            .iter()
            .find(|child| child.name == "Caches")
            .unwrap();

        assert_eq!(caches.safety, StorageSafety::Safe);
        assert_eq!(caches.clean_kind, CleanKind::Cache);
    }

    #[test]
    fn system_data_categories_include_safety_metadata() {
        let scan = scan_system_data(ScanMode::Fast);
        let caches = scan
            .categories
            .iter()
            .find(|category| category.name == "Caches")
            .unwrap();
        assert_eq!(caches.safety, StorageSafety::Safe);
        assert_eq!(caches.clean_kind, CleanKind::Cache);
    }

    #[test]
    fn percent_rounds_to_one_decimal() {
        assert_eq!(percent(1, 3), 33.3);
    }

    #[test]
    fn cargo_git_checkouts_are_safe_app_cleanup() {
        let path = Path::new("/Users/test/.cargo/git/checkouts");
        assert_eq!(
            classify_storage_path(path, true).safety,
            StorageSafety::Safe
        );
        assert!(app_cleanup_allowed(path));
    }

    #[test]
    fn known_node_cache_roots_are_safe_but_corepack_is_not() {
        for path in [
            Path::new("/Users/example/.npm"),
            Path::new("/Users/example/.yarn/cache"),
            Path::new("/Users/example/.yarn/berry/cache"),
            Path::new("/Users/example/.local/share/pnpm/store"),
        ] {
            assert_eq!(app_cleanup_safety(path), StorageSafety::Safe, "{path:?}");
            assert!(app_cleanup_allowed(path), "{path:?}");
        }
        assert!(!app_cleanup_allowed(Path::new(
            "/Users/example/.cache/node/corepack"
        )));
    }

    #[test]
    fn stremio_cache_descendants_inherit_cache_safety() {
        let path = Path::new(
            "/Users/test/Library/Application Support/stremio-server/stremio-cache/hash/video",
        );
        assert_eq!(
            classify_storage_path(path, true).safety,
            StorageSafety::Safe
        );
        assert!(app_cleanup_allowed(path));
    }

    #[test]
    fn ordinary_application_support_remains_user_data() {
        let path = Path::new("/Users/test/Library/Application Support/Example/Documents");
        assert_eq!(
            classify_storage_path(path, true).safety,
            StorageSafety::UserData
        );
        assert!(!app_cleanup_allowed(path));
    }

    #[test]
    fn cleanup_classification_fixture_matrix() {
        let fixtures = [
            ("/Users/test/Library/Caches/App/data", StorageSafety::Safe),
            ("/Users/test/Library/Logs/App.log", StorageSafety::Safe),
            ("/Users/test/.cargo/registry/cache/pkg", StorageSafety::Safe),
            ("/Users/test/.cargo/registry/src/pkg", StorageSafety::Safe),
            ("/Users/test/project/node_modules/pkg", StorageSafety::Safe),
            (
                "/Users/test/Library/Developer/Xcode/DerivedData/App",
                StorageSafety::Safe,
            ),
            (
                "/Users/test/Library/Containers/com.docker.docker/Data",
                StorageSafety::Review,
            ),
            (
                "/Users/test/Library/Application Support/MobileSync/Backup/device",
                StorageSafety::Review,
            ),
            ("/Users/test/project/.venv", StorageSafety::Review),
            ("/Users/test/Downloads/installer.dmg", StorageSafety::Review),
            (
                "/Users/test/Library/Application Support/Example/Documents",
                StorageSafety::UserData,
            ),
            ("/System/Library/CoreServices", StorageSafety::Protected),
        ];

        for (raw_path, expected) in fixtures {
            let path = Path::new(raw_path);
            assert_eq!(
                classify_storage_path(path, true).safety,
                expected,
                "unexpected classification for {raw_path}"
            );
        }
    }

    #[test]
    fn similarly_named_folders_do_not_inherit_cleanup_eligibility() {
        for raw_path in [
            "/Users/test/Documents/Caches/important",
            "/Users/test/project/.cargo/config.toml",
            "/Users/test/project/.vscode/settings.json",
            "/Users/test/Library/Application Support/stremio-server/stremio-cache-backup",
        ] {
            assert!(
                !matches!(
                    classify_storage_path(Path::new(raw_path), true).safety,
                    StorageSafety::Safe
                ),
                "{raw_path} must not be automatically eligible for cleanup"
            );
        }
    }
}
