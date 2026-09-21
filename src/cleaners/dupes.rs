use crate::core::{AnalysisResult, CleanKind, RiskLevel};
use crate::ui::{self, format_size};
use anyhow::Result;
use colored::Colorize;
use indicatif::{ProgressBar, ProgressStyle};
use rayon::Scope;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::cmp::Reverse;
use std::collections::{HashMap, HashSet};
use std::fmt;
use std::io::Read;
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DuplicateScanStage {
    Discovering,
    Hashing,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct DuplicateScanProgress {
    pub stage: DuplicateScanStage,
    pub scanned_files: u64,
    pub candidate_files: u64,
    pub processed_candidate_files: u64,
    pub hashed_files: u64,
    pub error_count: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DuplicateScanCancelled;

impl fmt::Display for DuplicateScanCancelled {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("duplicate scan cancelled")
    }
}

impl std::error::Error for DuplicateScanCancelled {}

#[derive(Debug, Clone, Serialize)]
pub struct DuplicateFile {
    pub path: PathBuf,
    pub size_bytes: u64,
    pub modified_at: u64,
}

#[derive(Debug, Clone, Serialize)]
pub struct DuplicateGroup {
    pub id: String,
    pub size_bytes: u64,
    pub wasted_bytes: u64,
    pub files: Vec<DuplicateFile>,
}

#[derive(Debug, Clone, Serialize)]
pub struct DuplicateReport {
    pub schema_version: u32,
    pub root: PathBuf,
    pub min_bytes: u64,
    pub scanned_files: u64,
    pub hashed_files: u64,
    pub partial: bool,
    pub error_count: u64,
    pub total_wasted_bytes: u64,
    pub elapsed_ms: u64,
    pub groups: Vec<DuplicateGroup>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct DuplicateCleanupRequest {
    pub groups: Vec<DuplicateCleanupGroupRequest>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct DuplicateCleanupGroupRequest {
    pub id: String,
    pub keeper_path: PathBuf,
    pub files: Vec<PathBuf>,
    pub selected: Vec<DuplicateCleanupSelection>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct DuplicateCleanupSelection {
    pub path: PathBuf,
    #[serde(default)]
    pub review_token: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct DuplicateCleanupGroupOutcome {
    pub id: String,
    pub keeper_path: PathBuf,
    pub valid: bool,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct DuplicateCleanupItemOutcome {
    pub group_id: String,
    pub keeper_path: PathBuf,
    pub path: PathBuf,
    pub moved: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub trash_path: Option<PathBuf>,
    pub error: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub review_token: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct DuplicateCleanupResponse {
    pub dry_run: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub session_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub receipt_error: Option<String>,
    pub moved_count: usize,
    pub failed_count: usize,
    pub total_bytes: u64,
    pub moved_bytes: u64,
    pub reclaimed_bytes: u64,
    pub groups: Vec<DuplicateCleanupGroupOutcome>,
    pub outcomes: Vec<DuplicateCleanupItemOutcome>,
}

pub fn analyze(root: &Path, min_bytes: u64) -> Result<DuplicateReport> {
    analyze_with_control(root, min_bytes, |_| {}, || false)
}

pub fn analyze_with_control<P, C>(
    root: &Path,
    min_bytes: u64,
    mut on_progress: P,
    should_cancel: C,
) -> Result<DuplicateReport>
where
    P: FnMut(DuplicateScanProgress) + Send,
    C: Fn() -> bool + Sync,
{
    let started_at = std::time::Instant::now();
    let root = std::fs::canonicalize(root)?;
    if !root.is_dir() {
        anyhow::bail!("Duplicate scan root is not a directory: {}", root.display());
    }

    let mut by_size: HashMap<u64, Vec<PathBuf>> = HashMap::new();
    let mut scanned_files = 0_u64;
    let mut error_count = 0_u64;
    on_progress(DuplicateScanProgress {
        stage: DuplicateScanStage::Discovering,
        scanned_files,
        candidate_files: 0,
        processed_candidate_files: 0,
        hashed_files: 0,
        error_count,
    });
    for entry in WalkDir::new(&root).follow_links(false) {
        cancel_if_requested(&should_cancel)?;
        let entry = match entry {
            Ok(entry) => entry,
            Err(_) => {
                error_count += 1;
                on_progress(DuplicateScanProgress {
                    stage: DuplicateScanStage::Discovering,
                    scanned_files,
                    candidate_files: 0,
                    processed_candidate_files: 0,
                    hashed_files: 0,
                    error_count,
                });
                continue;
            }
        };
        if !entry.file_type().is_file() {
            continue;
        }
        scanned_files += 1;
        match entry.metadata() {
            Ok(metadata) if metadata.len() >= min_bytes => {
                by_size
                    .entry(metadata.len())
                    .or_default()
                    .push(entry.into_path());
            }
            Ok(_) => {}
            Err(_) => error_count += 1,
        }
        on_progress(DuplicateScanProgress {
            stage: DuplicateScanStage::Discovering,
            scanned_files,
            candidate_files: 0,
            processed_candidate_files: 0,
            hashed_files: 0,
            error_count,
        });
    }

    let candidates: Vec<_> = by_size
        .into_iter()
        .filter(|(_, paths)| paths.len() > 1)
        .collect();
    let candidate_files = candidates.iter().fold(0_u64, |total, (_, paths)| {
        total.saturating_add(paths.len() as u64)
    });
    let mut by_hash: HashMap<String, Vec<(PathBuf, u64)>> = HashMap::new();
    let mut hashed_files = 0_u64;
    let mut processed_candidate_files = 0_u64;
    on_progress(DuplicateScanProgress {
        stage: DuplicateScanStage::Hashing,
        scanned_files,
        candidate_files,
        processed_candidate_files,
        hashed_files,
        error_count,
    });
    let work: Vec<_> = candidates
        .into_iter()
        .flat_map(|(size, paths)| paths.into_iter().map(move |path| (path, size)))
        .collect();
    let (sender, receiver) = std::sync::mpsc::channel();
    rayon::scope(|scope: &Scope<'_>| -> Result<()> {
        for (path, size) in work {
            let sender = sender.clone();
            let should_cancel = &should_cancel;
            scope.spawn(move |_| {
                let result = hash_file_with_cancel(&path, should_cancel);
                let _ = sender.send((path, size, result));
            });
        }
        drop(sender);

        for (path, size, result) in receiver {
            match result? {
                Some(hash) => {
                    hashed_files += 1;
                    by_hash.entry(hash).or_default().push((path, size));
                }
                None => error_count += 1,
            }
            processed_candidate_files += 1;
            on_progress(DuplicateScanProgress {
                stage: DuplicateScanStage::Hashing,
                scanned_files,
                candidate_files,
                processed_candidate_files,
                hashed_files,
                error_count,
            });
        }
        Ok(())
    })?;

    let mut groups: Vec<DuplicateGroup> = by_hash
        .into_iter()
        .filter(|(_, files)| files.len() > 1)
        .map(|(id, mut files)| {
            files.sort_by(|a, b| a.0.cmp(&b.0));
            let size_bytes = files[0].1;
            DuplicateGroup {
                id,
                size_bytes,
                wasted_bytes: size_bytes.saturating_mul(files.len().saturating_sub(1) as u64),
                files: files
                    .into_iter()
                    .map(|(path, size_bytes)| DuplicateFile {
                        modified_at: modified_secs(&path),
                        path,
                        size_bytes,
                    })
                    .collect(),
            }
        })
        .collect();
    groups.sort_by_key(|group| Reverse(group.wasted_bytes));
    let total_wasted_bytes = groups.iter().fold(0_u64, |total, group| {
        total.saturating_add(group.wasted_bytes)
    });

    Ok(DuplicateReport {
        schema_version: 1,
        root,
        min_bytes,
        scanned_files,
        hashed_files,
        partial: error_count > 0,
        error_count,
        total_wasted_bytes,
        elapsed_ms: started_at.elapsed().as_millis().min(u128::from(u64::MAX)) as u64,
        groups,
    })
}

struct PreparedDuplicate {
    group_id: String,
    keeper_path: PathBuf,
    item: crate::core::CleanItem,
    identity: crate::core::safety::FileIdentity,
}

/// Validates a duplicate cleanup as one transaction-sized preflight. Invalid
/// groups never contribute executable items, while other groups may proceed.
/// Execution re-runs this entire preflight and then the shared Trash executor
/// revalidates each identity immediately before moving it.
pub fn cleanup(request: DuplicateCleanupRequest, dry_run: bool) -> DuplicateCleanupResponse {
    let mut prepared = Vec::new();
    let mut group_outcomes = Vec::new();
    let mut item_outcomes = Vec::new();
    let mut selected_globally = HashSet::new();

    for group in request.groups {
        match prepare_group(&group, !dry_run, &mut selected_globally) {
            Ok(mut group_items) => {
                group_outcomes.push(DuplicateCleanupGroupOutcome {
                    id: group.id.clone(),
                    keeper_path: group.keeper_path.clone(),
                    valid: true,
                    error: None,
                });
                prepared.append(&mut group_items);
            }
            Err(error) => {
                group_outcomes.push(DuplicateCleanupGroupOutcome {
                    id: group.id.clone(),
                    keeper_path: group.keeper_path.clone(),
                    valid: false,
                    error: Some(error.clone()),
                });
                for selection in group.selected {
                    item_outcomes.push(DuplicateCleanupItemOutcome {
                        group_id: group.id.clone(),
                        keeper_path: group.keeper_path.clone(),
                        path: selection.path,
                        moved: false,
                        trash_path: None,
                        error: Some(error.clone()),
                        review_token: None,
                    });
                }
            }
        }
    }

    let total_bytes = prepared
        .iter()
        .map(|prepared| prepared.item.size_bytes)
        .sum();
    let (session_id, receipt_error) = if dry_run {
        for prepared in &prepared {
            let token = crate::core::safety::issue_review_token(&prepared.identity);
            item_outcomes.push(DuplicateCleanupItemOutcome {
                group_id: prepared.group_id.clone(),
                keeper_path: prepared.keeper_path.clone(),
                path: prepared.item.path.clone(),
                moved: false,
                trash_path: None,
                error: token.as_ref().err().cloned(),
                review_token: token.ok(),
            });
        }
        (None, None)
    } else {
        let items: Vec<_> = prepared.iter().map(|entry| entry.item.clone()).collect();
        let identities: Vec<_> = prepared
            .iter()
            .map(|entry| entry.identity.clone())
            .collect();
        let result = crate::core::trash::trash_reviewed_clean_items_with_session(
            "duplicates",
            &items,
            &identities,
        );
        for (prepared, outcome) in prepared.iter().zip(result.outcomes) {
            item_outcomes.push(DuplicateCleanupItemOutcome {
                group_id: prepared.group_id.clone(),
                keeper_path: prepared.keeper_path.clone(),
                path: outcome.path,
                moved: outcome.moved,
                trash_path: outcome.trash_path,
                error: outcome.error,
                review_token: None,
            });
        }
        (Some(result.session_id), result.receipt_error)
    };

    let moved_count = item_outcomes.iter().filter(|item| item.moved).count();
    let failed_count = item_outcomes
        .iter()
        .filter(|item| item.error.is_some())
        .count();
    let moved_bytes = prepared
        .iter()
        .filter(|entry| {
            item_outcomes
                .iter()
                .any(|outcome| outcome.path == entry.item.path && outcome.moved)
        })
        .map(|entry| entry.item.size_bytes)
        .sum();

    DuplicateCleanupResponse {
        dry_run,
        session_id,
        receipt_error,
        moved_count,
        failed_count,
        total_bytes,
        moved_bytes,
        reclaimed_bytes: 0,
        groups: group_outcomes,
        outcomes: item_outcomes,
    }
}

fn prepare_group(
    group: &DuplicateCleanupGroupRequest,
    require_tokens: bool,
    selected_globally: &mut HashSet<PathBuf>,
) -> Result<Vec<PreparedDuplicate>, String> {
    if group.files.len() < 2 {
        return Err("A duplicate group must contain at least two files; rescan first.".into());
    }
    if group.selected.is_empty() {
        return Err("Select at least one duplicate copy to move to Trash.".into());
    }

    let mut files = Vec::with_capacity(group.files.len());
    let mut unique_files = HashSet::new();
    for path in &group.files {
        let path = crate::core::safety::resolve_existing_path(path)?;
        if !path.is_file() {
            return Err(format!("Duplicate is no longer a file: {}", path.display()));
        }
        if !unique_files.insert(path.clone()) {
            return Err(
                "Duplicate group contains the same file more than once; rescan first.".into(),
            );
        }
        files.push(path);
    }
    let keeper = crate::core::safety::resolve_existing_path(&group.keeper_path)?;
    if !unique_files.contains(&keeper) {
        return Err("The declared keeper is missing from this duplicate group.".into());
    }

    let mut selected_paths = Vec::with_capacity(group.selected.len());
    let mut unique_selected = HashSet::new();
    for selection in &group.selected {
        let path = crate::core::safety::resolve_existing_path(&selection.path)?;
        if !unique_files.contains(&path) {
            return Err(format!(
                "Selected path is not part of this duplicate group: {}",
                path.display()
            ));
        }
        if !unique_selected.insert(path.clone()) {
            return Err("The same duplicate copy was selected more than once.".into());
        }
        selected_paths.push((path, selection.review_token.as_deref()));
    }
    if unique_selected.len() >= unique_files.len() {
        return Err("Refusing to select every copy in a duplicate group.".into());
    }
    if unique_selected.contains(&keeper) {
        return Err("The declared keeper cannot also be selected for cleanup.".into());
    }

    // Hash every member, not only the selected copies. The scan hash is the
    // group id, so content drift in the keeper or any unselected copy blocks
    // the entire group before the first destructive operation.
    for path in &files {
        let hash = hash_file_exact(path)?;
        if hash != group.id {
            return Err(format!(
                "Duplicate content changed after scan: {}; rescan before cleanup.",
                path.display()
            ));
        }
    }

    if unique_selected
        .iter()
        .any(|path| selected_globally.contains(path))
    {
        return Err("A duplicate copy was selected in more than one group.".into());
    }

    let mut prepared = Vec::with_capacity(selected_paths.len());
    for (path, token) in selected_paths {
        crate::core::safety::validate_removal(&path)?;
        let identity = if require_tokens {
            let token = token.ok_or_else(|| {
                "Missing duplicate review token; review duplicates again before cleanup."
                    .to_string()
            })?;
            let identity = crate::core::safety::parse_review_token(token)?;
            crate::core::safety::validate_identity(&path, &identity)?;
            identity
        } else {
            crate::core::safety::capture_identity(&path)?
        };
        let size_bytes = path
            .metadata()
            .map_err(|error| format!("Could not inspect {}: {}", path.display(), error))?
            .len();
        prepared.push(PreparedDuplicate {
            group_id: group.id.clone(),
            keeper_path: keeper.clone(),
            item: crate::core::CleanItem {
                label: path.display().to_string(),
                path,
                size_bytes,
                removable: true,
                kind: CleanKind::Duplicate,
                risk: RiskLevel::High,
                reason: format!("Content-identical duplicate; keeping {}.", keeper.display()),
            },
            identity,
        });
    }
    selected_globally.extend(unique_selected);
    Ok(prepared)
}

pub fn run(
    min_mb: u64,
    scan_path: Option<PathBuf>,
    trash: bool,
    keep: &str,
    dry_run: bool,
    yes: bool,
) -> Result<()> {
    let root = scan_path.unwrap_or_else(|| dirs::home_dir().unwrap_or_else(|| PathBuf::from(".")));
    let min_bytes = min_mb * 1024 * 1024;

    println!(
        "{}",
        format!(
            "Scanning {} for duplicates >= {} MB...",
            root.display(),
            min_mb
        )
        .dimmed()
    );

    let pb = ProgressBar::new_spinner();
    pb.set_style(
        ProgressStyle::with_template("{spinner:.cyan} Scanning and hashing files...")
            .unwrap()
            .progress_chars("=>-"),
    );

    pb.enable_steady_tick(std::time::Duration::from_millis(100));
    let report = analyze(&root, min_bytes)?;
    pb.finish_and_clear();

    if report.groups.is_empty() {
        println!("{}", "No duplicate files found.".green());
        return Ok(());
    }

    let groups: Vec<Vec<(PathBuf, u64)>> = report
        .groups
        .iter()
        .map(|group| {
            group
                .files
                .iter()
                .map(|file| (file.path.clone(), file.size_bytes))
                .collect()
        })
        .collect();

    let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("/"));

    let mut table = comfy_table::Table::new();
    table.load_preset(comfy_table::presets::UTF8_BORDERS_ONLY);
    table.set_header(vec!["Duplicate Files", "Size", "Copies", "Wasted"]);

    let mut total_wasted: u64 = 0;
    for group in &groups {
        let size = group[0].1;
        let wasted = size * (group.len() as u64 - 1);
        total_wasted += wasted;

        let first_path = group[0]
            .0
            .strip_prefix(&home)
            .map(|p| format!("~/{}", p.display()))
            .unwrap_or_else(|_| group[0].0.display().to_string());

        table.add_row(vec![
            first_path,
            format_size(size),
            group.len().to_string(),
            format_size(wasted),
        ]);

        for (path, _) in &group[1..] {
            let label = path
                .strip_prefix(&home)
                .map(|p| format!("  -> ~/{}", p.display()))
                .unwrap_or_else(|_| format!("  -> {}", path.display()));
            table.add_row(vec![label, String::new(), String::new(), String::new()]);
        }
    }

    println!("\n{}", "[ Duplicate Files ]".cyan().bold());
    println!("{}", table);
    println!("  Total wasted: {}", format_size(total_wasted).bold());
    if !trash {
        println!(
            "  {} duplicate group(s). Use --trash to move duplicate copies to Trash.",
            groups.len()
        );
        return Ok(());
    }
    if dry_run {
        ui::print_warn("Dry run -- duplicate files were not moved to Trash.");
        return Ok(());
    }

    let keep = KeepStrategy::parse(keep);
    let mut analysis = AnalysisResult::default();
    for group in &groups {
        let keep_path = keep.select(group);
        for (path, size) in group {
            if path == keep_path {
                continue;
            }
            analysis.add_with_meta(
                path.display().to_string(),
                path.clone(),
                *size,
                CleanKind::Duplicate,
                RiskLevel::High,
                format!(
                    "Content-identical duplicate; keeping {} by {} strategy.",
                    keep_path.display(),
                    keep.label()
                ),
            );
        }
    }

    if !yes
        && !ui::confirm(
            &format!("Move {} duplicate file(s) to Trash?", analysis.items.len()),
            false,
        )?
    {
        return Ok(());
    }

    for (path, outcome) in crate::core::trash::trash_clean_items("dupes", &analysis.items) {
        match outcome {
            Ok(_) => ui::print_ok(&format!("Moved to Trash: {}", path.display())),
            Err(e) => ui::print_warn(&format!("{}: {}", path.display(), e)),
        }
    }

    Ok(())
}

#[derive(Clone, Copy)]
enum KeepStrategy {
    First,
    Newest,
    Oldest,
    ShortestPath,
}

impl KeepStrategy {
    fn parse(value: &str) -> Self {
        match value {
            "newest" => Self::Newest,
            "oldest" => Self::Oldest,
            "shortest" | "shortest-path" => Self::ShortestPath,
            _ => Self::First,
        }
    }

    fn label(self) -> &'static str {
        match self {
            Self::First => "first",
            Self::Newest => "newest",
            Self::Oldest => "oldest",
            Self::ShortestPath => "shortest-path",
        }
    }

    fn select(self, group: &[(PathBuf, u64)]) -> &PathBuf {
        match self {
            Self::First => &group[0].0,
            Self::Newest => {
                &group
                    .iter()
                    .max_by_key(|(path, _)| modified_secs(path))
                    .unwrap_or(&group[0])
                    .0
            }
            Self::Oldest => {
                &group
                    .iter()
                    .min_by_key(|(path, _)| modified_secs(path))
                    .unwrap_or(&group[0])
                    .0
            }
            Self::ShortestPath => {
                &group
                    .iter()
                    .min_by_key(|(path, _)| path.components().count())
                    .unwrap_or(&group[0])
                    .0
            }
        }
    }
}

fn modified_secs(path: &Path) -> u64 {
    path.metadata()
        .ok()
        .and_then(|m| m.modified().ok())
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

fn cancel_if_requested<C>(should_cancel: &C) -> Result<()>
where
    C: Fn() -> bool,
{
    if should_cancel() {
        return Err(DuplicateScanCancelled.into());
    }
    Ok(())
}

fn hash_file_with_cancel<C>(path: &Path, should_cancel: &C) -> Result<Option<String>>
where
    C: Fn() -> bool + Sync,
{
    let mut file = match std::fs::File::open(path) {
        Ok(file) => file,
        Err(_) => return Ok(None),
    };
    let mut hasher = Sha256::new();
    let mut buffer = [0_u8; 256 * 1024];
    loop {
        cancel_if_requested(should_cancel)?;
        let read = match file.read(&mut buffer) {
            Ok(read) => read,
            Err(_) => return Ok(None),
        };
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    Ok(Some(format!("{:x}", hasher.finalize())))
}

fn hash_file_exact(path: &Path) -> Result<String, String> {
    let mut file = std::fs::File::open(path)
        .map_err(|error| format!("Could not open {}: {}", path.display(), error))?;
    let mut hasher = Sha256::new();
    let mut buffer = [0_u8; 256 * 1024];
    loop {
        let read = file
            .read(&mut buffer)
            .map_err(|error| format!("Could not read {}: {}", path.display(), error))?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    Ok(format!("{:x}", hasher.finalize()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    fn hash(bytes: &[u8]) -> String {
        let mut hasher = Sha256::new();
        hasher.update(bytes);
        format!("{:x}", hasher.finalize())
    }

    fn cleanup_group(
        id: String,
        keeper_path: PathBuf,
        files: Vec<PathBuf>,
        selected: Vec<(PathBuf, Option<String>)>,
    ) -> DuplicateCleanupGroupRequest {
        DuplicateCleanupGroupRequest {
            id,
            keeper_path,
            files,
            selected: selected
                .into_iter()
                .map(|(path, review_token)| DuplicateCleanupSelection { path, review_token })
                .collect(),
        }
    }

    #[test]
    fn analysis_groups_only_content_identical_files() {
        let dir = tempdir().unwrap();
        std::fs::write(dir.path().join("a.bin"), b"same bytes").unwrap();
        std::fs::write(dir.path().join("b.bin"), b"same bytes").unwrap();
        std::fs::write(dir.path().join("c.bin"), b"different!").unwrap();

        let report = analyze(dir.path(), 1).unwrap();

        assert_eq!(report.scanned_files, 3);
        assert_eq!(report.groups.len(), 1);
        assert_eq!(report.groups[0].files.len(), 2);
        assert_eq!(report.groups[0].wasted_bytes, 10);
        assert!(!report.partial);
    }

    #[test]
    fn analysis_respects_minimum_size() {
        let dir = tempdir().unwrap();
        std::fs::write(dir.path().join("a.bin"), b"same").unwrap();
        std::fs::write(dir.path().join("b.bin"), b"same").unwrap();

        let report = analyze(dir.path(), 5).unwrap();

        assert!(report.groups.is_empty());
        assert_eq!(report.hashed_files, 0);
    }

    #[test]
    fn progress_reports_discovery_then_measurable_hashing() {
        let dir = tempdir().unwrap();
        std::fs::write(dir.path().join("a.bin"), b"same bytes").unwrap();
        std::fs::write(dir.path().join("b.bin"), b"same bytes").unwrap();
        std::fs::write(dir.path().join("unique.bin"), b"unique content").unwrap();

        let mut updates = Vec::new();
        let report =
            analyze_with_control(dir.path(), 1, |progress| updates.push(progress), || false)
                .unwrap();

        assert_eq!(
            updates.first().unwrap().stage,
            DuplicateScanStage::Discovering
        );
        let hashing: Vec<_> = updates
            .iter()
            .filter(|progress| progress.stage == DuplicateScanStage::Hashing)
            .collect();
        assert!(!hashing.is_empty());
        assert_eq!(hashing[0].candidate_files, 2);
        assert_eq!(hashing[0].processed_candidate_files, 0);
        assert_eq!(hashing.last().unwrap().processed_candidate_files, 2);
        assert_eq!(hashing.last().unwrap().hashed_files, 2);
        assert_eq!(report.hashed_files, 2);
    }

    #[test]
    fn cancellation_stops_during_discovery_with_typed_error() {
        use std::sync::atomic::{AtomicBool, Ordering};

        let dir = tempdir().unwrap();
        std::fs::write(dir.path().join("a.bin"), b"same bytes").unwrap();
        std::fs::write(dir.path().join("b.bin"), b"same bytes").unwrap();
        let cancelled = AtomicBool::new(false);

        let error = analyze_with_control(
            dir.path(),
            1,
            |progress| {
                if progress.stage == DuplicateScanStage::Discovering && progress.scanned_files == 1
                {
                    cancelled.store(true, Ordering::Relaxed);
                }
            },
            || cancelled.load(Ordering::Relaxed),
        )
        .unwrap_err();

        assert!(error.downcast_ref::<DuplicateScanCancelled>().is_some());
        assert_eq!(error.to_string(), "duplicate scan cancelled");
    }

    #[test]
    fn cancellation_stops_before_hashing_candidates() {
        use std::sync::atomic::{AtomicBool, Ordering};

        let dir = tempdir().unwrap();
        std::fs::write(dir.path().join("a.bin"), b"same bytes").unwrap();
        std::fs::write(dir.path().join("b.bin"), b"same bytes").unwrap();
        let cancelled = AtomicBool::new(false);

        let error = analyze_with_control(
            dir.path(),
            1,
            |progress| {
                if progress.stage == DuplicateScanStage::Hashing {
                    assert_eq!(progress.candidate_files, 2);
                    assert_eq!(progress.processed_candidate_files, 0);
                    cancelled.store(true, Ordering::Relaxed);
                }
            },
            || cancelled.load(Ordering::Relaxed),
        )
        .unwrap_err();

        assert!(error.downcast_ref::<DuplicateScanCancelled>().is_some());
    }

    #[test]
    fn duplicate_cleanup_requires_a_keeper_in_the_group() {
        let dir = tempdir().unwrap();
        let a = dir.path().join("a");
        let b = dir.path().join("b");
        let outsider = dir.path().join("outsider");
        for path in [&a, &b, &outsider] {
            std::fs::write(path, b"same").unwrap();
        }
        let response = cleanup(
            DuplicateCleanupRequest {
                groups: vec![cleanup_group(
                    hash(b"same"),
                    outsider,
                    vec![a.clone(), b.clone()],
                    vec![(b, None)],
                )],
            },
            true,
        );

        assert!(!response.groups[0].valid);
        assert!(response.groups[0]
            .error
            .as_deref()
            .unwrap()
            .contains("keeper is missing"));
    }

    #[test]
    fn duplicate_cleanup_rejects_selecting_every_copy() {
        let dir = tempdir().unwrap();
        let a = dir.path().join("a");
        let b = dir.path().join("b");
        std::fs::write(&a, b"same").unwrap();
        std::fs::write(&b, b"same").unwrap();
        let response = cleanup(
            DuplicateCleanupRequest {
                groups: vec![cleanup_group(
                    hash(b"same"),
                    a.clone(),
                    vec![a.clone(), b.clone()],
                    vec![(a, None), (b, None)],
                )],
            },
            true,
        );

        assert!(!response.groups[0].valid);
        assert!(response.groups[0]
            .error
            .as_deref()
            .unwrap()
            .contains("every copy"));
    }

    #[test]
    fn duplicate_cleanup_rehashes_all_files_before_execution() {
        let dir = tempdir().unwrap();
        let keeper = dir.path().join("keeper");
        let selected = dir.path().join("selected");
        std::fs::write(&keeper, b"same").unwrap();
        std::fs::write(&selected, b"same").unwrap();
        let group_id = hash(b"same");
        let review = cleanup(
            DuplicateCleanupRequest {
                groups: vec![cleanup_group(
                    group_id.clone(),
                    keeper.clone(),
                    vec![keeper.clone(), selected.clone()],
                    vec![(selected.clone(), None)],
                )],
            },
            true,
        );
        let token = review.outcomes[0].review_token.clone().unwrap();

        // In-place modification preserves the inode, so this specifically
        // proves content is rehashed in addition to identity validation.
        std::fs::write(&keeper, b"drift").unwrap();
        let execution = cleanup(
            DuplicateCleanupRequest {
                groups: vec![cleanup_group(
                    group_id,
                    keeper.clone(),
                    vec![keeper, selected.clone()],
                    vec![(selected, Some(token))],
                )],
            },
            false,
        );

        assert_eq!(execution.moved_count, 0);
        assert!(!execution.groups[0].valid);
        assert!(execution.groups[0]
            .error
            .as_deref()
            .unwrap()
            .contains("content changed"));
    }

    #[test]
    fn duplicate_cleanup_rejects_replaced_inode_even_with_same_content() {
        let dir = tempdir().unwrap();
        let keeper = dir.path().join("keeper");
        let selected = dir.path().join("selected");
        std::fs::write(&keeper, b"same").unwrap();
        std::fs::write(&selected, b"same").unwrap();
        let group_id = hash(b"same");
        let review = cleanup(
            DuplicateCleanupRequest {
                groups: vec![cleanup_group(
                    group_id.clone(),
                    keeper.clone(),
                    vec![keeper.clone(), selected.clone()],
                    vec![(selected.clone(), None)],
                )],
            },
            true,
        );
        let token = review.outcomes[0].review_token.clone().unwrap();
        std::fs::remove_file(&selected).unwrap();
        std::fs::write(&selected, b"same").unwrap();

        let execution = cleanup(
            DuplicateCleanupRequest {
                groups: vec![cleanup_group(
                    group_id,
                    keeper.clone(),
                    vec![keeper, selected.clone()],
                    vec![(selected, Some(token))],
                )],
            },
            false,
        );

        assert_eq!(execution.moved_count, 0);
        assert!(execution.groups[0]
            .error
            .as_deref()
            .unwrap()
            .contains("changed after review"));
    }

    #[test]
    fn invalid_group_does_not_block_a_valid_group_dry_run() {
        let dir = tempdir().unwrap();
        let a = dir.path().join("a");
        let b = dir.path().join("b");
        let c = dir.path().join("c");
        let d = dir.path().join("d");
        for path in [&a, &b, &c, &d] {
            std::fs::write(path, b"same").unwrap();
        }
        let response = cleanup(
            DuplicateCleanupRequest {
                groups: vec![
                    cleanup_group(
                        hash(b"same"),
                        a.clone(),
                        vec![a, b.clone()],
                        vec![(b, None)],
                    ),
                    cleanup_group(
                        hash(b"same"),
                        c.clone(),
                        vec![c.clone(), d.clone()],
                        vec![(c, None), (d, None)],
                    ),
                ],
            },
            true,
        );

        assert!(response.groups[0].valid);
        assert!(!response.groups[1].valid);
        assert_eq!(response.outcomes.len(), 3);
        assert_eq!(
            response
                .outcomes
                .iter()
                .filter(|item| item.review_token.is_some())
                .count(),
            1
        );
        assert_eq!(response.failed_count, 2);
    }
}
