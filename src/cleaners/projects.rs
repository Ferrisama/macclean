use crate::core::fs::dir_size;
use crate::core::{AnalysisResult, CleanKind, Cleaner, RiskLevel};
use crate::ui;
use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::time::SystemTime;
use walkdir::WalkDir;

pub struct ProjectsCleaner;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectScanOptions {
    pub root: PathBuf,
    pub max_depth: usize,
    pub only: Vec<String>,
    pub exclude: Vec<PathBuf>,
    pub older_than_days: Option<u64>,
}

impl Default for ProjectScanOptions {
    fn default() -> Self {
        Self {
            root: dirs::home_dir().unwrap_or_else(|| PathBuf::from("/tmp")),
            max_depth: 5,
            only: Vec::new(),
            exclude: Vec::new(),
            older_than_days: None,
        }
    }
}

struct ArtifactDef {
    dir_name: &'static str,
    label: &'static str,
    aliases: &'static [&'static str],
}

const SKIP_DIRS: &[&str] = &[".git", ".ssh", "Library", "Applications", ".Trash"];

const ARTIFACTS: &[ArtifactDef] = &[
    ArtifactDef {
        dir_name: "node_modules",
        label: "Node.js dependencies",
        aliases: &["node", "npm", "yarn", "pnpm"],
    },
    ArtifactDef {
        dir_name: ".venv",
        label: "Python virtualenv",
        aliases: &["python", "venv", "virtualenv"],
    },
    ArtifactDef {
        dir_name: "venv",
        label: "Python virtualenv",
        aliases: &["python", "virtualenv"],
    },
    ArtifactDef {
        dir_name: "env",
        label: "Python virtualenv",
        aliases: &["python", "virtualenv"],
    },
    ArtifactDef {
        dir_name: "__pycache__",
        label: "Python bytecode cache",
        aliases: &["python", "pycache"],
    },
    ArtifactDef {
        dir_name: "build",
        label: "Build output",
        aliases: &["build"],
    },
    ArtifactDef {
        dir_name: "dist",
        label: "Distribution output",
        aliases: &["dist"],
    },
    ArtifactDef {
        dir_name: "target",
        label: "Rust/Java build output",
        aliases: &["rust", "cargo", "java", "maven", "gradle"],
    },
    ArtifactDef {
        dir_name: ".next",
        label: "Next.js build cache",
        aliases: &["next", "nextjs", "node"],
    },
    ArtifactDef {
        dir_name: ".nuxt",
        label: "Nuxt.js build cache",
        aliases: &["nuxt", "node"],
    },
    ArtifactDef {
        dir_name: ".parcel-cache",
        label: "Parcel cache",
        aliases: &["parcel", "node"],
    },
    ArtifactDef {
        dir_name: ".turbo",
        label: "Turborepo cache",
        aliases: &["turbo", "turborepo", "node"],
    },
];

impl Cleaner for ProjectsCleaner {
    fn name(&self) -> &str {
        "projects"
    }
    fn display_name(&self) -> &str {
        "Project Artifacts"
    }

    fn analyze(&self) -> Result<AnalysisResult> {
        analyze_with_options(&ProjectScanOptions::default())
    }

    fn clean(&self, result: &AnalysisResult, dry_run: bool, yes: bool) -> Result<()> {
        clean_result(result, dry_run, yes)
    }
}

pub fn run(options: ProjectScanOptions, dry_run: bool, yes: bool) -> Result<()> {
    let result = analyze_with_options(&options)?;
    clean_result(&result, dry_run, yes)
}

pub fn run_with_risk_ceiling(
    options: ProjectScanOptions,
    risk_ceiling: RiskLevel,
    dry_run: bool,
    yes: bool,
) -> Result<()> {
    let mut result = analyze_with_options(&options)?;
    result.items.retain(|item| item.risk <= risk_ceiling);
    clean_result(&result, dry_run, yes)
}

pub fn analyze_with_options(options: &ProjectScanOptions) -> Result<AnalysisResult> {
    let root = normalize_path(&options.root);
    let excludes: Vec<PathBuf> = options.exclude.iter().map(|p| normalize_path(p)).collect();
    let only: Vec<String> = options.only.iter().map(|s| s.to_lowercase()).collect();
    let mut result = AnalysisResult::default();

    for entry in WalkDir::new(&root)
        .max_depth(options.max_depth)
        .follow_links(false)
        .into_iter()
        .filter_entry(|entry| should_descend(entry.path(), &excludes))
        .filter_map(|e| e.ok())
    {
        if !entry.file_type().is_dir() {
            continue;
        }

        let name = entry.file_name().to_string_lossy();
        if SKIP_DIRS.iter().any(|s| *s == name.as_ref()) {
            continue;
        }

        let Some(artifact) = artifact_for_name(&name) else {
            continue;
        };
        if !matches_only(artifact, &only) {
            continue;
        }

        let path = entry.into_path();
        if excludes.iter().any(|exclude| path.starts_with(exclude)) {
            continue;
        }

        let age_days = age_days(&path);
        if let Some(min_age) = options.older_than_days {
            if age_days.is_some_and(|age| age < min_age) {
                continue;
            }
        }

        let size = dir_size(&path);
        if size == 0 {
            continue;
        }

        let rel = path
            .strip_prefix(&root)
            .map(|p| p.display().to_string())
            .unwrap_or_else(|_| path.display().to_string());
        let project_hint = project_hint(&path);
        let age_hint = age_days
            .map(|age| format!(", {}d old", age))
            .unwrap_or_default();

        result.add_with_meta(
            format!("{} -- {}{}", artifact.label, rel, age_hint),
            path,
            size,
            CleanKind::DevArtifact,
            risk_for_artifact(artifact),
            format!(
                "{} Generated artifact can usually be rebuilt from source/dependencies.",
                project_hint
            ),
        );
    }

    Ok(result)
}

fn clean_result(result: &AnalysisResult, dry_run: bool, yes: bool) -> Result<()> {
    if result.items.is_empty() {
        println!("No project artifact directories found.");
        return Ok(());
    }

    let display_items: Vec<_> = result.items.iter().take(40).cloned().collect();
    ui::print_analysis("Project Artifacts", &display_items);
    if result.items.len() > 40 {
        ui::print_warn(&format!(
            "... and {} more (showing 40 of {})",
            result.items.len() - 40,
            result.items.len()
        ));
    }

    if dry_run {
        return Ok(());
    }
    if !yes
        && !ui::confirm(
            &format!(
                "Move all {} project artifact directories to Trash?",
                result.items.len()
            ),
            false,
        )?
    {
        return Ok(());
    }

    for (path, outcome) in crate::core::trash::trash_clean_items("projects", &result.items) {
        match outcome {
            Ok(_) => ui::print_ok(&format!("Moved to Trash: {}", path.display())),
            Err(e) => ui::print_warn(&format!("{}: {}", path.display(), e)),
        }
    }
    Ok(())
}

fn should_descend(path: &Path, excludes: &[PathBuf]) -> bool {
    let Some(name) = path.file_name().and_then(|n| n.to_str()) else {
        return true;
    };
    !SKIP_DIRS.contains(&name) && !excludes.iter().any(|exclude| path.starts_with(exclude))
}

fn artifact_for_name(name: &str) -> Option<&'static ArtifactDef> {
    ARTIFACTS.iter().find(|artifact| artifact.dir_name == name)
}

fn matches_only(artifact: &ArtifactDef, only: &[String]) -> bool {
    only.is_empty()
        || only.iter().any(|filter| {
            artifact.dir_name.eq_ignore_ascii_case(filter)
                || artifact.label.to_lowercase().contains(filter)
                || artifact.aliases.iter().any(|alias| alias == filter)
        })
}

fn risk_for_artifact(artifact: &ArtifactDef) -> RiskLevel {
    match artifact.dir_name {
        "node_modules" | "__pycache__" | ".next" | ".nuxt" | ".parcel-cache" | ".turbo" => {
            RiskLevel::Low
        }
        _ => RiskLevel::Medium,
    }
}

fn project_hint(path: &Path) -> String {
    let project = path.parent().unwrap_or(path);
    let hints = [
        ("package-lock.json", "npm lockfile found."),
        ("pnpm-lock.yaml", "pnpm lockfile found."),
        ("yarn.lock", "Yarn lockfile found."),
        ("Cargo.toml", "Cargo project found."),
        ("pyproject.toml", "Python project found."),
        ("requirements.txt", "Python requirements found."),
        ("build.gradle", "Gradle project found."),
        ("pom.xml", "Maven project found."),
    ];

    hints
        .iter()
        .find(|(file, _)| project.join(file).exists())
        .map(|(_, hint)| (*hint).to_string())
        .unwrap_or_else(|| "No lockfile/project manifest detected nearby.".to_string())
}

fn age_days(path: &Path) -> Option<u64> {
    let modified = path.metadata().ok()?.modified().ok()?;
    SystemTime::now()
        .duration_since(modified)
        .ok()
        .map(|d| d.as_secs() / 86_400)
}

fn normalize_path(path: &Path) -> PathBuf {
    let expanded = expand_home(path);
    if expanded.is_absolute() {
        expanded
    } else {
        std::env::current_dir()
            .unwrap_or_else(|_| PathBuf::from("."))
            .join(expanded)
    }
}

fn expand_home(path: &Path) -> PathBuf {
    let raw = path.to_string_lossy();
    if raw == "~" {
        return dirs::home_dir().unwrap_or_else(|| path.to_path_buf());
    }
    if let Some(rest) = raw.strip_prefix("~/") {
        if let Some(home) = dirs::home_dir() {
            return home.join(rest);
        }
    }
    path.to_path_buf()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn only_filter_matches_alias() {
        let artifact = artifact_for_name("node_modules").unwrap();
        assert!(matches_only(artifact, &["node".into()]));
        assert!(!matches_only(artifact, &["python".into()]));
    }

    #[test]
    fn analyze_respects_root_and_only_filter() {
        let dir = tempdir().unwrap();
        let project = dir.path().join("app");
        fs::create_dir_all(project.join("node_modules/pkg")).unwrap();
        fs::create_dir_all(project.join(".venv/lib")).unwrap();
        fs::write(project.join("package-lock.json"), b"{}").unwrap();
        fs::write(project.join("node_modules/pkg/index.js"), b"data").unwrap();
        fs::write(project.join(".venv/lib/site.py"), b"data").unwrap();

        let result = analyze_with_options(&ProjectScanOptions {
            root: dir.path().to_path_buf(),
            max_depth: 4,
            only: vec!["node".into()],
            exclude: Vec::new(),
            older_than_days: None,
        })
        .unwrap();

        assert_eq!(result.items.len(), 1);
        assert!(result.items[0].label.contains("Node.js"));
        assert!(result.items[0].reason.contains("npm lockfile"));
    }

    #[test]
    fn analyze_respects_excludes() {
        let dir = tempdir().unwrap();
        let keep = dir.path().join("keep");
        fs::create_dir_all(keep.join("target/debug")).unwrap();
        fs::write(keep.join("target/debug/app"), b"data").unwrap();

        let result = analyze_with_options(&ProjectScanOptions {
            root: dir.path().to_path_buf(),
            max_depth: 4,
            only: Vec::new(),
            exclude: vec![keep],
            older_than_days: None,
        })
        .unwrap();

        assert!(result.items.is_empty());
    }
}
