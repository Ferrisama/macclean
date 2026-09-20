use anyhow::{bail, Result};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::core::{safety, CleanItem, CleanKind, RiskLevel};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlanItem {
    pub label: String,
    pub path: PathBuf,
    pub size_bytes: u64,
    pub is_dir: bool,
    pub kind: CleanKind,
    pub risk: RiskLevel,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoredPlan {
    pub name: String,
    pub created_at: u64,
    pub source: String,
    pub items: Vec<PlanItem>,
}

impl StoredPlan {
    pub fn total_bytes(&self) -> u64 {
        self.items.iter().map(|item| item.size_bytes).sum()
    }
}

pub fn create_from_paths(name: &str, paths: &[PathBuf]) -> Result<StoredPlan> {
    if paths.is_empty() {
        bail!("Provide at least one path.");
    }

    let mut items = Vec::new();
    for path in paths {
        let resolved_path = safety::resolve_existing_path(path)
            .map_err(|e| anyhow::anyhow!("{}: {}", path.display(), e))?;
        safety::validate_removal(&resolved_path)
            .map_err(|e| anyhow::anyhow!("{}: {}", path.display(), e))?;
        let is_dir = resolved_path.is_dir();
        let size_bytes = if is_dir {
            crate::core::fs::dir_size(&resolved_path)
        } else {
            resolved_path.metadata().map(|m| m.len()).unwrap_or(0)
        };
        items.push(PlanItem {
            label: resolved_path.display().to_string(),
            // A plan may be applied from a different working directory than
            // the one in which it was created. Persisting the resolved path
            // makes its target stable in that case.
            path: resolved_path,
            size_bytes,
            is_dir,
            kind: CleanKind::Unknown,
            risk: RiskLevel::High,
            reason: "User-preselected path; review before applying.".into(),
        });
    }

    Ok(StoredPlan {
        name: name.to_string(),
        created_at: now_secs(),
        source: "manual paths".into(),
        items,
    })
}

pub fn create_from_clean_items(
    name: &str,
    source: &str,
    clean_items: &[CleanItem],
) -> Result<StoredPlan> {
    let mut items = Vec::new();
    for item in clean_items.iter().filter(|item| item.removable) {
        let resolved_path = safety::resolve_existing_path(&item.path)
            .map_err(|e| anyhow::anyhow!("{}: {}", item.path.display(), e))?;
        safety::validate_removal(&resolved_path)
            .map_err(|e| anyhow::anyhow!("{}: {}", item.path.display(), e))?;
        items.push(PlanItem {
            label: item.label.clone(),
            path: resolved_path.clone(),
            size_bytes: item.size_bytes,
            is_dir: resolved_path.is_dir(),
            kind: item.kind,
            risk: item.risk,
            reason: item.reason.clone(),
        });
    }

    if items.is_empty() {
        bail!("No removable items to save in plan.");
    }

    Ok(StoredPlan {
        name: name.to_string(),
        created_at: now_secs(),
        source: source.into(),
        items,
    })
}

pub fn save(plan: &StoredPlan) -> Result<PathBuf> {
    validate_name(&plan.name)?;
    let path = plan_path(&plan.name)?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    let data = serde_json::to_string_pretty(plan)?;
    fs::write(&path, data)?;
    Ok(path)
}

pub fn load(name: &str) -> Result<StoredPlan> {
    validate_name(name)?;
    let path = plan_path(name)?;
    let data = fs::read_to_string(&path)
        .map_err(|e| anyhow::anyhow!("Could not read plan '{}': {}", name, e))?;
    Ok(serde_json::from_str(&data)?)
}

pub fn remove(name: &str) -> Result<()> {
    validate_name(name)?;
    let path = plan_path(name)?;
    if !path.exists() {
        bail!("Plan '{}' does not exist.", name);
    }
    fs::remove_file(path)?;
    Ok(())
}

pub fn list() -> Result<Vec<String>> {
    let dir = plans_dir()?;
    if !dir.exists() {
        return Ok(Vec::new());
    }
    let mut names = Vec::new();
    for entry in fs::read_dir(dir)?.filter_map(|e| e.ok()) {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) == Some("json") {
            if let Some(name) = path.file_stem().and_then(|n| n.to_str()) {
                names.push(name.to_string());
            }
        }
    }
    names.sort();
    Ok(names)
}

pub fn apply(name: &str, dry_run: bool) -> Result<Vec<(PathBuf, Result<(), String>)>> {
    let plan = load(name)?;
    validate_plan(&plan)?;
    if dry_run {
        return Ok(plan
            .items
            .iter()
            .map(|item| (item.path.clone(), Ok(())))
            .collect());
    }

    let items: Vec<CleanItem> = plan
        .items
        .iter()
        .map(|item| CleanItem {
            label: item.label.clone(),
            path: item.path.clone(),
            size_bytes: item.size_bytes,
            removable: true,
            kind: item.kind,
            risk: item.risk,
            reason: item.reason.clone(),
        })
        .collect();
    Ok(crate::core::trash::trash_clean_items(
        &format!("plan:{}", name),
        &items,
    ))
}

pub fn validate_plan(plan: &StoredPlan) -> Result<()> {
    if plan.items.is_empty() {
        bail!("Plan '{}' has no items.", plan.name);
    }
    for item in &plan.items {
        if !item.path.exists() {
            bail!("Planned path no longer exists: {}", item.path.display());
        }
        if item.path.is_dir() != item.is_dir {
            bail!(
                "Planned path changed type since save: {}",
                item.path.display()
            );
        }
        safety::validate_removal(&item.path)
            .map_err(|e| anyhow::anyhow!("{}: {}", item.path.display(), e))?;
    }
    Ok(())
}

fn plan_path(name: &str) -> Result<PathBuf> {
    Ok(plans_dir()?.join(format!("{}.json", name)))
}

fn plans_dir() -> Result<PathBuf> {
    let Some(home) = dirs::home_dir() else {
        bail!("Could not find home directory.");
    };
    Ok(home.join("Library/Application Support/macclean/plans"))
}

fn validate_name(name: &str) -> Result<()> {
    if name.is_empty()
        || !name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
    {
        bail!("Plan names may only contain letters, numbers, '-' and '_'.");
    }
    Ok(())
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
    use std::path::Path;
    use tempfile::tempdir;

    #[test]
    fn rejects_bad_names() {
        assert!(validate_name("../bad").is_err());
        assert!(validate_name("good-name_1").is_ok());
    }

    #[test]
    fn validates_changed_type() {
        let plan = StoredPlan {
            name: "test".into(),
            created_at: 0,
            source: "test".into(),
            items: vec![PlanItem {
                label: "missing".into(),
                path: Path::new("/tmp/macclean_missing_plan_item").to_path_buf(),
                size_bytes: 0,
                is_dir: false,
                kind: CleanKind::Unknown,
                risk: RiskLevel::High,
                reason: "test".into(),
            }],
        };
        assert!(validate_plan(&plan).is_err());
    }

    #[test]
    fn created_plans_store_resolved_paths() {
        let dir = tempdir().unwrap();
        let target = dir.path().join("removable.log");
        fs::write(&target, "data").unwrap();

        let plan = create_from_paths("resolved", std::slice::from_ref(&target)).unwrap();

        assert!(plan.items[0].path.is_absolute());
        assert_eq!(plan.items[0].path, fs::canonicalize(target).unwrap());
    }
}
