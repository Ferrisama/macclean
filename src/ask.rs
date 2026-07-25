use anyhow::Result;
use std::path::PathBuf;

use crate::cleaners;
use crate::ui;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AskAction {
    SystemData,
    LibraryTree,
    OldNodeProjects,
    DuplicatePhotos,
    LargeDownloads,
    DeepUninstall { app_name: String },
    SafeDevClean,
}

impl AskAction {
    pub fn command(&self) -> String {
        match self {
            AskAction::SystemData => "macclean system-data".into(),
            AskAction::LibraryTree => "macclean system-data --path ~/Library --depth 1".into(),
            AskAction::OldNodeProjects => {
                "macclean projects --only node --older-than-days 30".into()
            }
            AskAction::DuplicatePhotos => "macclean dupes --path ~/Pictures".into(),
            AskAction::LargeDownloads => "macclean largest --path ~/Downloads".into(),
            AskAction::DeepUninstall { app_name } => {
                format!("macclean uninstall '{}' --deep", app_name)
            }
            AskAction::SafeDevClean => "macclean profile run dev-safe".into(),
        }
    }
}

pub fn run(query: &[String], dry_run: bool, yes: bool) -> Result<()> {
    let phrase = query.join(" ");
    if phrase.trim().is_empty() {
        ui::print_warn("Ask needs a phrase, e.g. macclean ask \"why is system data huge\"");
        return Ok(());
    }

    let Some(action) = resolve(&phrase) else {
        ui::print_warn("I could not map that request to a safe offline command.");
        println!("Try:");
        println!("  macclean ask \"why is system data huge\"");
        println!("  macclean ask \"clean old node projects\"");
        println!("  macclean ask \"find duplicate photos\"");
        println!("  macclean ask \"uninstall chrome deeply\"");
        return Ok(());
    };

    println!("Planned command: {}", action.command());
    if !yes && !ui::confirm("Run this command?", false)? {
        println!("  Aborted.");
        return Ok(());
    }

    execute(action, dry_run)
}

pub fn resolve(phrase: &str) -> Option<AskAction> {
    let normalized = normalize(phrase);

    if contains_any(
        &normalized,
        &["tree library", "library tree", "storage tree"],
    ) {
        return Some(AskAction::LibraryTree);
    }

    if contains_any(
        &normalized,
        &[
            "system data",
            "why full",
            "why is my mac full",
            "storage huge",
            "storage full",
            "disk full",
        ],
    ) {
        return Some(AskAction::SystemData);
    }

    if contains_any(
        &normalized,
        &["safe dev clean", "dev safe clean", "safe developer clean"],
    ) {
        return Some(AskAction::SafeDevClean);
    }

    if normalized.contains("node")
        && contains_any(&normalized, &["old", "stale", "clean", "remove", "delete"])
        && contains_any(
            &normalized,
            &["project", "projects", "node_modules", "node modules"],
        )
    {
        return Some(AskAction::OldNodeProjects);
    }

    if contains_any(
        &normalized,
        &["duplicate photo", "duplicate photos", "duplicate pictures"],
    ) {
        return Some(AskAction::DuplicatePhotos);
    }

    if normalized.contains("large") && normalized.contains("download") {
        return Some(AskAction::LargeDownloads);
    }

    if contains_any(&normalized, &["uninstall", "remove app", "delete app"]) {
        return Some(AskAction::DeepUninstall {
            app_name: extract_app_name(&normalized),
        });
    }

    None
}

fn execute(action: AskAction, dry_run: bool) -> Result<()> {
    match action {
        AskAction::SystemData => cleaners::system_data::run(None, 2, 8),
        AskAction::LibraryTree => cleaners::system_data::run(Some(home_path("Library")), 1, 8),
        AskAction::OldNodeProjects => cleaners::projects::run(
            cleaners::projects::ProjectScanOptions {
                root: dirs::home_dir().unwrap_or_else(|| PathBuf::from("/tmp")),
                max_depth: 5,
                only: vec!["node".into()],
                exclude: Vec::new(),
                older_than_days: Some(30),
            },
            dry_run,
            true,
        ),
        AskAction::DuplicatePhotos => cleaners::dupes::run(
            10,
            Some(home_path("Pictures")),
            false,
            "first",
            dry_run,
            true,
        ),
        AskAction::LargeDownloads => {
            cleaners::largest::run(100, 30, Some(home_path("Downloads")), false, dry_run, true)
        }
        AskAction::DeepUninstall { app_name } => cleaners::uninstall::run_with_options(
            cleaners::uninstall::UninstallOptions {
                app_name: Some(app_name),
                app_path: None,
                bundle_id: None,
                deep: true,
            },
            dry_run,
            true,
        ),
        AskAction::SafeDevClean => match crate::core::profile::load_project_profile("dev-safe") {
            Ok(profile) => cleaners::projects::run_with_risk_ceiling(
                profile.options,
                profile.risk_ceiling,
                dry_run,
                true,
            ),
            Err(_) => {
                ui::print_warn("Profile 'dev-safe' does not exist.");
                println!(
                    "Create it with: macclean profile create-project dev-safe --path ~/code --only node,rust,python --older-than-days 30"
                );
                Ok(())
            }
        },
    }
}

fn normalize(phrase: &str) -> String {
    phrase
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c.is_whitespace() {
                c.to_ascii_lowercase()
            } else {
                ' '
            }
        })
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

fn contains_any(value: &str, needles: &[&str]) -> bool {
    needles.iter().any(|needle| value.contains(needle))
}

fn extract_app_name(normalized: &str) -> String {
    if normalized.contains("chrome") {
        return "Google Chrome".into();
    }
    if normalized.contains("firefox") {
        return "Firefox".into();
    }
    if normalized.contains("brave") {
        return "Brave Browser".into();
    }

    let stop_words = [
        "uninstall",
        "remove",
        "delete",
        "app",
        "application",
        "deep",
        "deeply",
        "fully",
        "all",
        "files",
    ];
    let words: Vec<_> = normalized
        .split_whitespace()
        .filter(|word| !stop_words.contains(word))
        .collect();
    if words.is_empty() {
        "Unknown".into()
    } else {
        words
            .into_iter()
            .map(title_case)
            .collect::<Vec<_>>()
            .join(" ")
    }
}

fn title_case(word: &str) -> String {
    let mut chars = word.chars();
    let Some(first) = chars.next() else {
        return String::new();
    };
    format!("{}{}", first.to_ascii_uppercase(), chars.as_str())
}

fn home_path(relative: &str) -> PathBuf {
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("/tmp"))
        .join(relative)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolves_system_data() {
        assert_eq!(
            resolve("why is system data huge"),
            Some(AskAction::SystemData)
        );
        assert_eq!(resolve("why full"), Some(AskAction::SystemData));
    }

    #[test]
    fn resolves_library_tree_before_system_data() {
        assert_eq!(resolve("show storage tree"), Some(AskAction::LibraryTree));
    }

    #[test]
    fn resolves_node_projects() {
        assert_eq!(
            resolve("clean old node projects"),
            Some(AskAction::OldNodeProjects)
        );
    }

    #[test]
    fn resolves_duplicate_photos() {
        assert_eq!(
            resolve("find duplicate photos"),
            Some(AskAction::DuplicatePhotos)
        );
    }

    #[test]
    fn resolves_deep_uninstall_chrome() {
        assert_eq!(
            resolve("uninstall chrome deeply"),
            Some(AskAction::DeepUninstall {
                app_name: "Google Chrome".into()
            })
        );
    }
}
