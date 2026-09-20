use crate::core::cmd::run_cmd;
use crate::core::fs::dir_size;
use crate::core::safety::{self, FileIdentity};
use crate::core::{CleanItem, CleanKind, RiskLevel};
use crate::ui::{confirm, format_size, print_err, print_ok, print_warn};
use anyhow::{bail, Result};
use colored::Colorize;
use comfy_table::{presets::UTF8_BORDERS_ONLY, Table};
use std::path::{Path, PathBuf};

/// One path found to belong to an app, with its size on disk.
#[derive(Debug, Clone, serde::Serialize)]
pub struct TraceItem {
    pub path: PathBuf,
    pub size_bytes: u64,
    pub reason: String,
    pub risk: RiskLevel,
    pub identity: FileIdentity,
}

/// The result of scanning for an app and everything associated with it,
/// built once and shared between the CLI and TUI front ends.
#[derive(Debug, Clone, serde::Serialize)]
pub struct UninstallPlan {
    pub app_name: String,
    pub bundle_id: String,
    pub bundle_id_guessed: bool,
    pub app_path: PathBuf,
    pub items: Vec<TraceItem>,
    pub total_size: u64,
    pub deep: bool,
    pub running_processes: Vec<RunningProcess>,
    pub can_execute: bool,
    pub preflight_error: Option<String>,
}

#[derive(Debug, Clone, serde::Serialize)]
pub struct RunningProcess {
    pub pid: u32,
    pub command: String,
}

#[derive(Debug, Clone)]
pub struct UninstallOptions {
    pub app_name: Option<String>,
    pub app_path: Option<PathBuf>,
    pub bundle_id: Option<String>,
    pub deep: bool,
}

/// List installed .app bundles in /Applications and ~/Applications as
/// (display name, path) pairs, for an app picker.
pub fn list_installed_apps() -> Vec<(String, PathBuf)> {
    let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
    let mut apps = Vec::new();
    for dir in [PathBuf::from("/Applications"), home.join("Applications")] {
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.filter_map(|e| e.ok()) {
            let name = entry.file_name().to_string_lossy().to_string();
            if let Some(stripped) = name.strip_suffix(".app") {
                apps.push((stripped.to_string(), entry.path()));
            }
        }
    }
    apps.sort_by_key(|a| a.0.to_lowercase());
    apps.dedup_by(|a, b| a.0.eq_ignore_ascii_case(&b.0));
    apps
}

/// Search /Applications and ~/Applications for an .app bundle whose
/// directory name matches (case-insensitive) `app_name`.
fn find_app(app_name: &str) -> Option<PathBuf> {
    let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
    let search_dirs = [PathBuf::from("/Applications"), home.join("Applications")];

    let lower = app_name.to_lowercase();

    for dir in &search_dirs {
        if !dir.exists() {
            continue;
        }
        let Ok(entries) = std::fs::read_dir(dir) else {
            continue;
        };
        for entry in entries.filter_map(|e| e.ok()) {
            let fname = entry.file_name();
            let name = fname.to_string_lossy().to_lowercase();
            if name == format!("{}.app", lower)
                || name == lower
                || name == format!("{}.app", lower.replace(' ', ""))
            {
                return Some(entry.path());
            }
        }
    }
    None
}

fn find_app_by_bundle_id(bundle_id: &str) -> Option<PathBuf> {
    let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
    for dir in [PathBuf::from("/Applications"), home.join("Applications")] {
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.filter_map(|e| e.ok()) {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) != Some("app") {
                continue;
            }
            if crate::core::plist::read_bundle_id(&path).as_deref() == Some(bundle_id) {
                return Some(path);
            }
        }
    }
    None
}

/// Scan a directory for sub-paths that match the bundle_id or app_name.
fn find_traces(search_dir: &Path, bundle_id: &str, app_name: &str) -> Vec<PathBuf> {
    if !search_dir.exists() {
        return Vec::new();
    }
    let Ok(entries) = std::fs::read_dir(search_dir) else {
        return Vec::new();
    };

    let bid_lower = bundle_id.to_lowercase();
    let name_lower = app_name.to_lowercase();

    entries
        .filter_map(|e| e.ok())
        .filter(|e| {
            let fname = e.file_name();
            let n = fname.to_string_lossy().to_lowercase();
            n.contains(&bid_lower) || n.contains(&name_lower)
        })
        .map(|e| e.path())
        .collect()
}

/// Find an app and everything associated with it, without printing or
/// touching anything. Shared by the CLI's `uninstall` command and the TUI's
/// Uninstall tab.
pub fn build_plan(app_name: &str) -> Result<UninstallPlan> {
    build_plan_with_options(UninstallOptions {
        app_name: Some(app_name.to_string()),
        app_path: None,
        bundle_id: None,
        deep: false,
    })
}

pub fn build_plan_with_options(options: UninstallOptions) -> Result<UninstallPlan> {
    let deep = options.deep;
    let app_path = if let Some(path) = options.app_path {
        path
    } else if let Some(bundle_id) = &options.bundle_id {
        find_app_by_bundle_id(bundle_id)
            .ok_or_else(|| anyhow::anyhow!("Could not find app with bundle ID '{}'.", bundle_id))?
    } else if let Some(app_name) = &options.app_name {
        match find_app(app_name) {
            Some(p) => p,
            None => bail!(
                "Could not find '{}' in /Applications or ~/Applications.",
                app_name
            ),
        }
    } else {
        bail!("Provide an app name, --path, or --bundle-id.");
    };

    if !app_path.exists() {
        bail!("App path does not exist: {}", app_path.display());
    }
    if app_path.extension().and_then(|e| e.to_str()) != Some("app") {
        bail!("App path is not a .app bundle: {}", app_path.display());
    }

    if is_protected_app(&app_path) {
        bail!(
            "Refusing to uninstall protected app: {}",
            app_path.display()
        );
    }

    let app_name = options.app_name.unwrap_or_else(|| {
        app_path
            .file_stem()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| "Unknown App".to_string())
    });

    let (bundle_id, bundle_id_guessed) = if let Some(bundle_id) = options.bundle_id {
        (bundle_id, false)
    } else {
        match crate::core::plist::read_bundle_id(&app_path) {
            Some(id) => (id, false),
            None => (app_name.to_lowercase().replace(' ', "."), true),
        }
    };

    let mut to_remove: Vec<(PathBuf, String, RiskLevel)> = vec![(
        app_path.clone(),
        "Application bundle selected for uninstall.".into(),
        RiskLevel::High,
    )];

    let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
    let base_scan_dirs = [
        home.join("Library/Application Support"),
        home.join("Library/Caches"),
        home.join("Library/Containers"),
        home.join("Library/Preferences"),
        home.join("Library/Logs"),
        home.join("Library/Saved Application State"),
    ];

    for dir in &base_scan_dirs {
        for path in find_traces(dir, &bundle_id, &app_name) {
            to_remove.push((
                path,
                format!(
                    "Matched app name or bundle ID '{}' in user Library.",
                    bundle_id
                ),
                RiskLevel::Medium,
            ));
        }
    }

    if deep {
        for (dir, reason, risk) in deep_scan_dirs(&home) {
            for path in find_traces(&dir, &bundle_id, &app_name) {
                to_remove.push((path, reason.to_string(), risk));
            }
        }

        for receipt in find_receipts(&bundle_id, &app_name) {
            to_remove.push((
                receipt,
                "Package receipt matched app name or bundle ID.".into(),
                RiskLevel::High,
            ));
        }
    }

    let mut canonical_targets = Vec::new();
    for (path, reason, risk) in to_remove {
        let identity = safety::capture_identity(&path)
            .map_err(|error| anyhow::anyhow!("{}: {}", path.display(), error))?;
        canonical_targets.push((identity.canonical_path.clone(), reason, risk, identity));
    }
    canonical_targets.sort_by(|a, b| {
        a.0.components()
            .count()
            .cmp(&b.0.components().count())
            .then_with(|| a.0.cmp(&b.0))
    });
    let mut deduplicated = Vec::new();
    for candidate in canonical_targets {
        if deduplicated.iter().any(
            |(parent, _, _, _): &(PathBuf, String, RiskLevel, FileIdentity)| {
                candidate.0 != *parent && candidate.0.starts_with(parent)
            },
        ) {
            continue;
        }
        if !deduplicated
            .iter()
            .any(|(path, _, _, _)| *path == candidate.0)
        {
            deduplicated.push(candidate);
        }
    }

    let mut total_size = 0u64;
    let items: Vec<TraceItem> = deduplicated
        .into_iter()
        .map(|(path, reason, risk, identity)| {
            let size_bytes = if path.is_dir() {
                dir_size(&path)
            } else {
                path.metadata().map(|m| m.len()).unwrap_or(0)
            };
            total_size += size_bytes;
            TraceItem {
                path,
                size_bytes,
                reason,
                risk,
                identity,
            }
        })
        .collect();

    let process_check = find_running_processes(&app_path);
    let (running_processes, preflight_error) = match process_check {
        ProcessCheck::Stopped => (Vec::new(), None),
        ProcessCheck::Running(processes) => (processes, None),
        ProcessCheck::Unknown(error) => (Vec::new(), Some(error)),
    };
    let can_execute = running_processes.is_empty() && preflight_error.is_none();

    Ok(UninstallPlan {
        app_name,
        bundle_id,
        bundle_id_guessed,
        app_path,
        items,
        total_size,
        deep,
        running_processes,
        can_execute,
        preflight_error,
    })
}

fn deep_scan_dirs(home: &Path) -> Vec<(PathBuf, &'static str, RiskLevel)> {
    vec![
        (
            home.join("Library/Group Containers"),
            "Deep scan matched Group Containers.",
            RiskLevel::High,
        ),
        (
            home.join("Library/Application Scripts"),
            "Deep scan matched Application Scripts.",
            RiskLevel::High,
        ),
        (
            home.join("Library/HTTPStorages"),
            "Deep scan matched HTTP storage.",
            RiskLevel::Medium,
        ),
        (
            home.join("Library/WebKit"),
            "Deep scan matched WebKit storage.",
            RiskLevel::Medium,
        ),
        (
            home.join("Library/Cookies"),
            "Deep scan matched cookies.",
            RiskLevel::Medium,
        ),
        (
            home.join("Library/LaunchAgents"),
            "Deep scan matched user LaunchAgent.",
            RiskLevel::High,
        ),
        (
            PathBuf::from("/Library/LaunchAgents"),
            "Deep scan matched system LaunchAgent.",
            RiskLevel::High,
        ),
        (
            PathBuf::from("/Library/LaunchDaemons"),
            "Deep scan matched LaunchDaemon.",
            RiskLevel::High,
        ),
        (
            PathBuf::from("/Library/PrivilegedHelperTools"),
            "Deep scan matched privileged helper.",
            RiskLevel::High,
        ),
        (
            PathBuf::from("/Library/Application Support"),
            "Deep scan matched system Application Support.",
            RiskLevel::High,
        ),
    ]
}

fn find_receipts(bundle_id: &str, app_name: &str) -> Vec<PathBuf> {
    let receipt_dir = Path::new("/private/var/db/receipts");
    if !receipt_dir.exists() {
        return Vec::new();
    }
    find_traces(receipt_dir, bundle_id, app_name)
        .into_iter()
        .filter(|path| {
            matches!(
                path.extension().and_then(|e| e.to_str()),
                Some("bom") | Some("plist")
            )
        })
        .collect()
}

pub fn is_protected_app(app_path: &Path) -> bool {
    if app_path.starts_with("/System") {
        return true;
    }
    let protected_names = [
        "Finder",
        "System Settings",
        "System Preferences",
        "Terminal",
        "Safari",
        "App Store",
    ];
    app_path
        .file_stem()
        .and_then(|n| n.to_str())
        .is_some_and(|name| protected_names.contains(&name))
}

/// Move everything in the plan to the Trash (recoverable). Returns one
/// result per item.
pub fn execute(plan: &UninstallPlan) -> Vec<(PathBuf, std::result::Result<(), String>)> {
    match find_running_processes(&plan.app_path) {
        ProcessCheck::Running(processes) => {
            return vec![(
                plan.app_path.clone(),
                Err(format!(
                    "refusing uninstall while {} matching process(es) are running",
                    processes.len()
                )),
            )];
        }
        ProcessCheck::Unknown(error) => {
            return vec![(plan.app_path.clone(), Err(error))];
        }
        ProcessCheck::Stopped => {}
    }

    let mut items = Vec::new();
    for item in &plan.items {
        let path = match safety::validate_identity(&item.path, &item.identity) {
            Ok(path) => path,
            Err(error) => return vec![(item.path.clone(), Err(error))],
        };
        items.push(CleanItem {
            label: item.path.display().to_string(),
            path,
            size_bytes: item.size_bytes,
            removable: true,
            kind: CleanKind::AppTrace,
            risk: item.risk,
            reason: item.reason.clone(),
        });
    }
    crate::core::trash::trash_clean_items("uninstall", &items)
}

pub fn run_with_options(options: UninstallOptions, dry_run: bool, yes: bool) -> Result<()> {
    let title = options
        .app_name
        .as_deref()
        .or(options.bundle_id.as_deref())
        .unwrap_or("selected app")
        .to_string();

    let plan = build_plan_with_options(options)?;
    println!("\n{}", format!("[ Uninstall: {} ]", title).cyan().bold());
    println!("  Found: {}", plan.app_path.display());
    if plan.bundle_id_guessed {
        print_warn(&format!(
            "Could not read Info.plist -- guessing bundle ID '{}'. Review the paths below carefully.",
            plan.bundle_id
        ));
    }
    println!("  Bundle ID: {}", plan.bundle_id);

    let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
    let mut table = Table::new();
    table.load_preset(UTF8_BORDERS_ONLY);
    table.set_header(vec!["Path", "Size", "Risk", "Why"]);
    for item in &plan.items {
        let label = item
            .path
            .strip_prefix(&home)
            .map(|p| format!("~/{}", p.display()))
            .unwrap_or_else(|_| item.path.display().to_string());
        table.add_row(vec![
            label,
            format_size(item.size_bytes),
            item.risk.label().to_string(),
            item.reason.clone(),
        ]);
    }
    println!("{}", table);
    println!("  Total: {}", format_size(plan.total_size).bold());

    if dry_run {
        print_warn("Dry run — nothing removed.");
        return Ok(());
    }

    warn_if_running(&plan);
    if !plan.can_execute {
        bail!("Quit {} before uninstalling it.", plan.app_name);
    }

    if !yes {
        let prompt = format!("Move {} and all its files to the Trash?", plan.app_name);
        if !confirm(&prompt, false)? {
            println!("  Aborted.");
            return Ok(());
        }
    }

    for (path, result) in execute(&plan) {
        match result {
            Ok(_) => print_ok(&format!("Moved to Trash: {}", path.display())),
            Err(e) => print_err(&format!("Failed to trash {}: {}", path.display(), e)),
        }
    }

    Ok(())
}

enum ProcessCheck {
    Running(Vec<RunningProcess>),
    Stopped,
    Unknown(String),
}

fn find_running_processes(app_path: &Path) -> ProcessCheck {
    let marker = format!("{}/Contents/", app_path.display());
    let result = run_cmd(&["ps", "-axo", "pid=,command="]);
    if !result.success() {
        return ProcessCheck::Unknown(format!(
            "could not verify whether the app is running: {}",
            result.output
        ));
    }
    let processes: Vec<_> = parse_running_processes(&result.output)
        .into_iter()
        .filter(|process| process.command.contains(&marker))
        .collect();
    if processes.is_empty() {
        ProcessCheck::Stopped
    } else {
        ProcessCheck::Running(processes)
    }
}

fn parse_running_processes(output: &str) -> Vec<RunningProcess> {
    output
        .lines()
        .filter_map(|line| {
            let (pid, command) = line.trim().split_once(char::is_whitespace)?;
            Some(RunningProcess {
                pid: pid.parse().ok()?,
                command: command.trim().to_string(),
            })
        })
        .collect()
}

fn warn_if_running(plan: &UninstallPlan) {
    if !plan.running_processes.is_empty() {
        print_warn(&format!(
            "{} appears to be running. Quit it before confirming uninstall.",
            plan.app_name
        ));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_running_process_lines() {
        let processes = parse_running_processes(
            "123 /Applications/Example.app/Contents/MacOS/Example --flag\n456 helper process\n",
        );
        assert_eq!(processes.len(), 2);
        assert_eq!(processes[0].pid, 123);
        assert!(processes[0].command.contains("Example.app"));
    }
}
