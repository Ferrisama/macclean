use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::cleaners;
use crate::cleaners::health::HealthSnapshot;
use crate::core::storage::{
    self, ScanMode, StorageNode, StorageSafety, StorageScan, SystemDataScan,
};
use crate::core::{CleanKind, RiskLevel};

const RECIPE_ITEM_LIMIT: usize = 30;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppScanOptions {
    pub root: PathBuf,
    pub depth: usize,
    pub limit: usize,
    pub mode: ScanMode,
    pub include_system_data: bool,
    pub include_health: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct AppScanProgress {
    pub stage: String,
    pub current_path: Option<PathBuf>,
    pub scanned_items: usize,
    pub estimated_items: usize,
    pub progress: f64,
    pub partial: bool,
    pub item: Option<AppScanItem>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppScan {
    pub schema_version: u32,
    pub root_scan: StorageScan,
    pub system_data: Option<SystemDataScan>,
    pub largest_items: Vec<AppScanItem>,
    pub cleanup_candidates: Vec<AppScanItem>,
    pub safety_totals: Vec<SafetyTotal>,
    pub health: Option<HealthSnapshot>,
}

#[derive(Debug, Clone, Serialize)]
pub struct AppRecipes {
    pub schema_version: u32,
    pub scanned_at: u64,
    pub elapsed_ms: u128,
    pub recipes: Vec<CleanupRecipe>,
}

#[derive(Debug, Clone, Serialize)]
pub struct CleanupRecipe {
    pub id: String,
    pub title: String,
    pub subtitle: String,
    pub safety: StorageSafety,
    pub total_bytes: u64,
    pub item_count: usize,
    pub selected_by_default: bool,
    pub command: String,
    pub items: Vec<RecipeItem>,
}

#[derive(Debug, Clone, Serialize)]
pub struct RecipeItem {
    pub label: String,
    pub path: PathBuf,
    pub size_bytes: u64,
    pub kind: CleanKind,
    pub risk: RiskLevel,
    pub reason: String,
    pub removable: bool,
    pub safety: StorageSafety,
    pub app_eligible: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AppScanItem {
    pub name: String,
    pub path: PathBuf,
    pub size_bytes: u64,
    pub percent_of_root: f64,
    pub is_dir: bool,
    pub partial: bool,
    pub safety: StorageSafety,
    pub clean_kind: CleanKind,
    pub cleanup_action: String,
    pub cleanup_reason: String,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub struct SafetyTotal {
    pub safety: StorageSafety,
    pub size_bytes: u64,
    pub percent_of_root: f64,
    pub item_count: usize,
}

pub fn scan(options: AppScanOptions) -> Result<AppScan> {
    scan_with_progress(options, |_| {})
}

pub fn scan_with_progress<F>(options: AppScanOptions, progress: F) -> Result<AppScan>
where
    F: Fn(AppScanProgress) + Sync,
{
    progress(AppScanProgress {
        stage: "Walking folders".into(),
        current_path: None,
        scanned_items: 0,
        estimated_items: 0,
        progress: 0.02,
        partial: false,
        item: None,
    });
    let storage_progress =
        |path: &std::path::Path, size: u64, partial: bool, scanned: usize, estimated: usize| {
            let node = storage::classified_node(path, size, path.is_dir(), partial);
            progress(AppScanProgress {
                stage: "Sizing folders".into(),
                current_path: Some(path.to_path_buf()),
                scanned_items: scanned,
                estimated_items: estimated,
                progress: if estimated == 0 {
                    0.1
                } else {
                    0.1 + (scanned as f64 / estimated as f64) * 0.65
                },
                partial,
                item: (size > 0).then(|| app_scan_item(&node, size)),
            });
        };
    let root_scan = storage::scan_tree_with_progress(
        options.root.clone(),
        options.depth,
        options.limit,
        options.mode,
        Some(&storage_progress),
    )?;
    progress(AppScanProgress {
        stage: "Classifying cleanup candidates".into(),
        current_path: None,
        scanned_items: root_scan.tree.children.len(),
        estimated_items: root_scan.tree.children.len(),
        progress: 0.8,
        partial: root_scan.partial,
        item: None,
    });
    let _ = storage::write_tree_cache(&root_scan);

    let system_data = options.include_system_data.then(|| {
        progress(AppScanProgress {
            stage: "Estimating System Data".into(),
            current_path: None,
            scanned_items: 0,
            estimated_items: 0,
            progress: 0.86,
            partial: false,
            item: None,
        });
        let scan = storage::scan_system_data(options.mode);
        let _ = storage::write_system_data_cache(&scan);
        scan
    });

    let safety_items = safety_items(&root_scan.tree, root_scan.tree.size_bytes);
    let mut largest_items: Vec<_> = root_scan
        .tree
        .children
        .iter()
        .map(|node| app_scan_item(node, root_scan.tree.size_bytes))
        .collect();
    largest_items.sort_by_key(|item| std::cmp::Reverse(item.size_bytes));
    largest_items.truncate(options.limit);

    let mut cleanup_candidates = cleanup_candidates(&root_scan.tree, root_scan.tree.size_bytes);
    cleanup_candidates.sort_by(|a, b| {
        safety_rank(a.safety)
            .cmp(&safety_rank(b.safety))
            .then_with(|| b.size_bytes.cmp(&a.size_bytes))
    });
    cleanup_candidates.truncate(options.limit);

    let safety_totals = safety_totals(&safety_items, root_scan.tree.size_bytes);
    let health = options.include_health.then(|| {
        progress(AppScanProgress {
            stage: "Collecting health".into(),
            current_path: None,
            scanned_items: 0,
            estimated_items: 0,
            progress: 0.94,
            partial: false,
            item: None,
        });
        crate::cleaners::health::snapshot()
    });

    let child_count = root_scan.tree.children.len();
    let root_partial = root_scan.partial;
    let scan = AppScan {
        schema_version: 1,
        root_scan,
        system_data,
        largest_items,
        cleanup_candidates,
        safety_totals,
        health,
    };
    let _ = write_app_scan_cache(&scan);
    progress(AppScanProgress {
        stage: if root_partial {
            "Complete (partial)".into()
        } else {
            "Complete".into()
        },
        current_path: None,
        scanned_items: child_count,
        estimated_items: child_count,
        progress: 1.0,
        partial: root_partial,
        item: None,
    });
    Ok(scan)
}

pub fn write_app_scan_cache(scan: &AppScan) -> Result<PathBuf> {
    let path = app_cache_dir()?.join("latest-app-scan.json");
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(&path, serde_json::to_string_pretty(scan)?)?;
    Ok(path)
}

pub fn read_app_scan_cache() -> Result<Option<AppScan>> {
    let path = app_cache_dir()?.join("latest-app-scan.json");
    if !path.exists() {
        return Ok(None);
    }
    let data = fs::read_to_string(path)?;
    Ok(Some(serde_json::from_str(&data)?))
}

pub fn recipes() -> AppRecipes {
    let start = std::time::Instant::now();
    let definitions = [
        RecipeDef {
            id: "safe-caches",
            title: "Safe Cache Cleanup",
            subtitle: "Regenerable app, browser, package, and crash-report cache data.",
            cleaners: &[
                "browser",
                "node",
                "pip",
                "cargo",
                "gradle",
                "maven",
                "go",
                "crash-reports",
            ],
            safety: StorageSafety::Safe,
            selected_by_default: true,
            command: "macclean quick/dev",
        },
        RecipeDef {
            id: "developer",
            title: "Developer Cleanup",
            subtitle: "Build output and package-manager caches for active development machines.",
            cleaners: &[
                "node", "pip", "cargo", "gradle", "maven", "go", "android", "xcode",
            ],
            safety: StorageSafety::Review,
            selected_by_default: false,
            command: "macclean dev",
        },
        RecipeDef {
            id: "docker",
            title: "Docker Cleanup",
            subtitle: "Stopped containers, unused images, volumes, and build cache.",
            cleaners: &["docker"],
            safety: StorageSafety::Review,
            selected_by_default: false,
            command: "macclean docker",
        },
        RecipeDef {
            id: "installers",
            title: "Downloads Installers",
            subtitle: "DMG, PKG, ZIP, and archive installers left after setup.",
            cleaners: &["installers"],
            safety: StorageSafety::Review,
            selected_by_default: false,
            command: "macclean installers",
        },
        RecipeDef {
            id: "apps",
            title: "App Leftovers",
            subtitle: "Ghost files from apps that appear to be uninstalled.",
            cleaners: &["apps"],
            safety: StorageSafety::Review,
            selected_by_default: false,
            command: "macclean apps",
        },
        RecipeDef {
            id: "stremio",
            title: "Stremio Cleanup",
            subtitle: "Streaming video, server, Chromium, and WebKit caches. Review because this may sign Stremio out.",
            cleaners: &["stremio"],
            safety: StorageSafety::Review,
            selected_by_default: false,
            command: "macclean stremio",
        },
    ];

    let mut recipes: Vec<_> = definitions.into_iter().map(build_recipe).collect();
    recipes.sort_by_key(|recipe| std::cmp::Reverse(recipe.total_bytes));
    AppRecipes {
        schema_version: 1,
        scanned_at: now_secs(),
        elapsed_ms: start.elapsed().as_millis(),
        recipes,
    }
}

struct RecipeDef {
    id: &'static str,
    title: &'static str,
    subtitle: &'static str,
    cleaners: &'static [&'static str],
    safety: StorageSafety,
    selected_by_default: bool,
    command: &'static str,
}

fn build_recipe(def: RecipeDef) -> CleanupRecipe {
    let mut items = Vec::new();
    for cleaner_name in def.cleaners {
        let Some(cleaner) = cleaners::cleaner_by_name(cleaner_name) else {
            continue;
        };
        let Ok(result) = cleaner.analyze() else {
            continue;
        };
        items.extend(result.items.into_iter().map(|item| {
            let safety = storage::app_cleanup_safety(&item.path);
            let app_eligible = item.removable && storage::app_cleanup_allowed(&item.path);
            RecipeItem {
                label: item.label,
                path: item.path,
                size_bytes: item.size_bytes,
                kind: item.kind,
                risk: item.risk,
                reason: item.reason,
                removable: item.removable,
                safety,
                app_eligible,
            }
        }));
    }

    let total_bytes = items
        .iter()
        .filter(|item| item.app_eligible)
        .map(|item| item.size_bytes)
        .sum();
    let item_count = items.iter().filter(|item| item.app_eligible).count();
    items.sort_by_key(|item| std::cmp::Reverse(item.size_bytes));
    items.truncate(RECIPE_ITEM_LIMIT);
    CleanupRecipe {
        id: def.id.into(),
        title: def.title.into(),
        subtitle: def.subtitle.into(),
        safety: def.safety,
        total_bytes,
        item_count,
        selected_by_default: def.selected_by_default,
        command: def.command.into(),
        items,
    }
}

fn app_cache_dir() -> Result<PathBuf> {
    let Some(home) = dirs::home_dir() else {
        anyhow::bail!("Could not find home directory.");
    };
    Ok(home.join("Library/Application Support/macclean/scans"))
}

fn safety_items(root: &StorageNode, root_size: u64) -> Vec<AppScanItem> {
    let mut items = Vec::new();
    for child in &root.children {
        collect_safety_items(child, root_size, &mut items);
    }
    items
}

fn collect_safety_items(node: &StorageNode, root_size: u64, items: &mut Vec<AppScanItem>) {
    if node.safety != StorageSafety::Unknown || node.children.is_empty() {
        items.push(app_scan_item(node, root_size));
        return;
    }
    for child in &node.children {
        collect_safety_items(child, root_size, items);
    }
}

fn cleanup_candidates(root: &StorageNode, root_size: u64) -> Vec<AppScanItem> {
    let mut items = Vec::new();
    for child in &root.children {
        collect_cleanup_candidates(child, root_size, &mut items);
    }
    items
}

fn collect_cleanup_candidates(node: &StorageNode, root_size: u64, items: &mut Vec<AppScanItem>) {
    if matches!(node.safety, StorageSafety::Safe | StorageSafety::Review) {
        items.push(app_scan_item(node, root_size));
        return;
    }
    for child in &node.children {
        collect_cleanup_candidates(child, root_size, items);
    }
}

fn app_scan_item(node: &StorageNode, root_size: u64) -> AppScanItem {
    AppScanItem {
        name: node.name.clone(),
        path: node.path.clone(),
        size_bytes: node.size_bytes,
        percent_of_root: percent(node.size_bytes, root_size),
        is_dir: node.is_dir,
        partial: node.partial,
        safety: node.safety,
        clean_kind: node.clean_kind,
        cleanup_action: node.cleanup_action.clone(),
        cleanup_reason: node.cleanup_reason.clone(),
    }
}

fn safety_totals(items: &[AppScanItem], root_size: u64) -> Vec<SafetyTotal> {
    let safeties = [
        StorageSafety::Safe,
        StorageSafety::Review,
        StorageSafety::UserData,
        StorageSafety::Protected,
        StorageSafety::Unknown,
    ];

    safeties
        .into_iter()
        .map(|safety| {
            let matching = items.iter().filter(|item| item.safety == safety);
            let mut size_bytes = 0u64;
            let mut item_count = 0usize;
            for item in matching {
                size_bytes = size_bytes.saturating_add(item.size_bytes);
                item_count += 1;
            }
            SafetyTotal {
                safety,
                size_bytes,
                percent_of_root: percent(size_bytes, root_size),
                item_count,
            }
        })
        .collect()
}

fn safety_rank(safety: StorageSafety) -> u8 {
    match safety {
        StorageSafety::Safe => 0,
        StorageSafety::Review => 1,
        StorageSafety::UserData => 2,
        StorageSafety::Protected => 3,
        StorageSafety::Unknown => 4,
    }
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
    fn app_scan_sorts_largest_items_and_candidates() {
        let dir = tempdir().unwrap();
        fs::create_dir_all(dir.path().join("Library/Caches/App")).unwrap();
        fs::create_dir_all(dir.path().join("Documents")).unwrap();
        fs::write(
            dir.path().join("Library/Caches/App/blob.bin"),
            vec![0u8; 2048],
        )
        .unwrap();
        fs::write(dir.path().join("Documents/file.txt"), vec![0u8; 1024]).unwrap();

        let scan = scan(AppScanOptions {
            root: dir.path().to_path_buf(),
            depth: 3,
            limit: 10,
            mode: ScanMode::Deep,
            include_system_data: false,
            include_health: false,
        })
        .unwrap();

        assert_eq!(scan.schema_version, 1);
        assert!(scan
            .largest_items
            .windows(2)
            .all(|items| items[0].size_bytes >= items[1].size_bytes));
        assert_eq!(scan.largest_items[0].name, "Library");
        assert!(scan
            .cleanup_candidates
            .iter()
            .any(|item| item.safety == StorageSafety::Safe));
        assert!(scan
            .safety_totals
            .iter()
            .any(|total| total.safety == StorageSafety::Safe && total.size_bytes > 0));
        assert!(scan.health.is_none());
    }
}
