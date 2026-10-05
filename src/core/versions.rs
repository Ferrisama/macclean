//! Installed-version review. Unknown usage is a blocker, never evidence of disuse.
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use walkdir::WalkDir;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VersionEntry {
    pub path: PathBuf,
    pub name: String,
    pub family: String,
    pub version: String,
    pub size_bytes: u64,
    pub removable: bool,
    pub reasons: Vec<String>,
}
#[derive(Debug, Serialize, Deserialize)]
pub struct VersionReport {
    pub entries: Vec<VersionEntry>,
    pub project_roots: Vec<PathBuf>,
    pub checked_files: usize,
    pub complete: bool,
    pub warnings: Vec<String>,
}
#[derive(Clone)]
struct Pin {
    family: &'static str,
    value: String,
    source: String,
}
const ROOTS: &[(&str, &str)] = &[
    ("Rust", ".rustup/toolchains"),
    ("Node", ".nvm/versions/node"),
    ("Python", ".pyenv/versions"),
    ("VS Code", ".vscode/extensions"),
    ("Cursor", ".cursor/extensions"),
];

/// Only direct installed-version directories enter the reviewed removal flow.
pub fn managed_home(path: &Path) -> Option<PathBuf> {
    let parent = path.parent()?;
    ROOTS.iter().find_map(|(_, relative)| {
        let mut base = parent.to_path_buf();
        for component in Path::new(relative).components().rev() {
            if base.file_name()? != component.as_os_str() {
                return None;
            }
            base.pop();
        }
        Some(base)
    })
}

pub fn is_manager_data(path: &Path) -> bool {
    path.components().any(|component| {
        matches!(
            component.as_os_str().to_str(),
            Some(".rustup" | ".nvm" | ".pyenv")
        )
    }) || path
        .components()
        .collect::<Vec<_>>()
        .windows(2)
        .any(|parts| {
            matches!(parts[0].as_os_str().to_str(), Some(".vscode" | ".cursor"))
                && parts[1].as_os_str() == "extensions"
        })
}

fn read(path: &Path) -> Result<String, String> {
    let metadata =
        std::fs::metadata(path).map_err(|_| format!("Cannot read {}", path.display()))?;
    if metadata.len() > 1_048_576 {
        return Err(format!("Pin file too large: {}", path.display()));
    }
    std::fs::read_to_string(path).map_err(|_| format!("Cannot read {}", path.display()))
}
fn pin(pins: &mut Vec<Pin>, family: &'static str, value: &str, source: &Path) {
    for value in value
        .split_whitespace()
        .filter(|value| !value.starts_with('#'))
    {
        pins.push(Pin {
            family,
            value: value.into(),
            source: source.display().to_string(),
        });
    }
}
fn matches_pin(version: &str, requirement: &str, family: &str) -> bool {
    let version = if family == "Node" {
        version.trim_start_matches('v')
    } else {
        version
    };
    let requirement = if family == "Node" {
        requirement.trim_start_matches('v')
    } else {
        requirement
    };
    if requirement == "system" {
        return false;
    }
    if version == requirement {
        return true;
    }
    if version
        .strip_prefix(requirement)
        .is_some_and(|tail| tail.starts_with('.') || tail.starts_with('-'))
    {
        return true;
    }
    // Ranges, aliases, paths and unknown formats may require any installed version.
    if family == "Rust" {
        return !requirement.starts_with(|ch: char| ch.is_ascii_digit())
            && !["stable", "beta", "nightly"]
                .iter()
                .any(|channel| requirement.starts_with(channel));
    }
    requirement
        .chars()
        .any(|ch| !ch.is_ascii_digit() && ch != '.')
}

fn jsonc(text: &str) -> Option<serde_json::Value> {
    let mut chars = text.chars().peekable();
    let mut clean = String::new();
    let mut quoted = false;
    let mut escaped = false;
    while let Some(ch) = chars.next() {
        if quoted {
            clean.push(ch);
            if escaped {
                escaped = false;
            } else if ch == '\\' {
                escaped = true;
            } else if ch == '"' {
                quoted = false;
            }
        } else if ch == '"' {
            quoted = true;
            clean.push(ch);
        } else if ch == '/' && chars.peek() == Some(&'/') {
            for ch in chars.by_ref() {
                if ch == '\n' {
                    clean.push(ch);
                    break;
                }
            }
        } else if ch == '/' && chars.peek() == Some(&'*') {
            chars.next();
            let mut closed = false;
            while let Some(ch) = chars.next() {
                if ch == '*' && chars.peek() == Some(&'/') {
                    chars.next();
                    closed = true;
                    break;
                }
            }
            if !closed {
                return None;
            }
            clean.push(' ');
        } else if ch == ','
            && chars
                .clone()
                .find(|ch| !ch.is_whitespace())
                .is_some_and(|ch| ch == ']' || ch == '}')
        {
            // JSONC allows trailing commas.
        } else {
            clean.push(ch);
        }
    }
    serde_json::from_str(&clean).ok()
}

fn gather_pins(home: &Path, extra: &[PathBuf]) -> (Vec<Pin>, VersionReport) {
    let mut report = VersionReport {
        entries: vec![],
        project_roots: vec![home.into()],
        checked_files: 0,
        complete: true,
        warnings: vec![],
    };
    for root in extra {
        if !root.is_dir() {
            report.complete = false;
            report
                .warnings
                .push(format!("Project root is unavailable: {}", root.display()));
        }
        let resolved = root.canonicalize().unwrap_or_else(|_| root.clone());
        if !report.project_roots.contains(&resolved) {
            report.project_roots.push(resolved);
        }
    }
    let mut pins = Vec::new();
    let mut visited = std::collections::HashSet::new();
    for root in &report.project_roots {
        let walker = WalkDir::new(root)
            .follow_links(false)
            .max_depth(16)
            .into_iter()
            .filter_entry(|entry| {
                if entry.depth() == 0 {
                    return true;
                }
                let name = entry.file_name().to_string_lossy();
                ![
                    ".git",
                    "target",
                    "node_modules",
                    ".venv",
                    "venv",
                    ".rustup",
                    ".nvm",
                    ".pyenv",
                    ".cache",
                    ".codex",
                    ".Trash",
                    ".ssh",
                    "Library",
                    "Applications",
                ]
                .contains(&name.as_ref())
                    && !(entry.path().parent() == Some(home)
                        && [".vscode", ".cursor"].contains(&name.as_ref()))
            });
        for (index, entry) in walker.enumerate() {
            if index >= 100_000 {
                report.complete = false;
                report
                    .warnings
                    .push("Project discovery reached its entry limit.".into());
                break;
            }
            let entry = match entry {
                Ok(entry) => entry,
                Err(_) => {
                    report.complete = false;
                    continue;
                }
            };
            if entry.depth() == 16 && entry.file_type().is_dir() {
                report.complete = false;
            }
            let path = entry.path();
            if entry.file_type().is_symlink() && path.is_dir() {
                if let Ok(target) = path.canonicalize() {
                    if !report
                        .project_roots
                        .iter()
                        .any(|root| target.starts_with(root))
                    {
                        report.complete = false;
                        report.warnings.push(format!(
                            "Add the external project root for this linked folder: {}",
                            path.display()
                        ));
                    }
                } else {
                    report.complete = false;
                }
            }
            let name = entry.file_name().to_string_lossy();
            if !entry.file_type().is_file() || !visited.insert(path.to_path_buf()) {
                continue;
            }
            let recognized = [
                "rust-toolchain",
                "rust-toolchain.toml",
                ".nvmrc",
                ".node-version",
                ".python-version",
                ".pyenv-version",
                ".tool-versions",
                "package.json",
            ]
            .contains(&name.as_ref())
                || (name == "extensions.json"
                    && path
                        .parent()
                        .and_then(Path::file_name)
                        .is_some_and(|name| name == ".vscode"));
            if !recognized {
                continue;
            }
            report.checked_files += 1;
            let text = match read(path) {
                Ok(text) => text,
                Err(error) => {
                    report.complete = false;
                    report.warnings.push(error);
                    continue;
                }
            };
            let parsed = match name.as_ref() {
                "rust-toolchain" | "rust-toolchain.toml" => {
                    if let Ok(value) = text.parse::<toml::Value>() {
                        if let Some(channel) = value
                            .get("toolchain")
                            .and_then(|value| value.get("channel"))
                            .and_then(|value| value.as_str())
                        {
                            pin(&mut pins, "Rust", channel, path);
                            true
                        } else if let Some(custom) = value
                            .get("toolchain")
                            .and_then(|value| value.get("path"))
                            .and_then(|value| value.as_str())
                        {
                            pin(&mut pins, "Rust", custom, path);
                            true
                        } else {
                            false
                        }
                    } else if name == "rust-toolchain" && text.split_whitespace().count() == 1 {
                        pin(&mut pins, "Rust", text.trim(), path);
                        true
                    } else {
                        false
                    }
                }
                ".nvmrc" | ".node-version" => {
                    pin(&mut pins, "Node", text.trim(), path);
                    !text.trim().is_empty()
                }
                ".python-version" | ".pyenv-version" => {
                    pin(&mut pins, "Python", text.trim(), path);
                    !text.trim().is_empty()
                }
                ".tool-versions" => {
                    let mut valid = true;
                    for line in text.lines() {
                        let mut fields = line.split_whitespace();
                        let family = match fields.next() {
                            Some("rust") => "Rust",
                            Some("nodejs" | "node") => "Node",
                            Some("python") => "Python",
                            _ => continue,
                        };
                        let versions: Vec<_> =
                            fields.take_while(|field| !field.starts_with('#')).collect();
                        if versions.is_empty() {
                            valid = false;
                        }
                        for version in versions {
                            pin(&mut pins, family, version, path);
                        }
                    }
                    valid
                }
                "package.json" => {
                    if let Ok(value) = serde_json::from_str::<serde_json::Value>(&text) {
                        if let Some(engine) = value
                            .get("engines")
                            .and_then(|value| value.get("node"))
                            .and_then(|value| value.as_str())
                        {
                            pin(&mut pins, "Node", engine, path);
                        }
                        true
                    } else {
                        false
                    }
                }
                "extensions.json" => {
                    // Exact version recommendations are kept even when this JSONC file cannot be parsed.
                    for token in text.split('"') {
                        if let Some((id, version)) = token.split_once('@') {
                            if id.contains('.') && !version.contains(char::is_whitespace) {
                                pin(&mut pins, "Extension", &format!("{id}@{version}"), path);
                            }
                        }
                    }
                    jsonc(&text).is_some_and(|value| value.is_object())
                }
                _ => true,
            };
            if !parsed {
                report.complete = false;
                report
                    .warnings
                    .push(format!("Unrecognized pin file: {}", path.display()));
            }
        }
    }
    if !report.complete {
        report.warnings.push("Project coverage is incomplete; version removal is blocked. Add accessible project roots or resolve unreadable files.".into());
    }
    (pins, report)
}

fn default_pins(home: &Path, family: &'static str, pins: &mut Vec<Pin>) -> bool {
    match family {
        "Rust" => {
            let path = home.join(".rustup/settings.toml");
            let Some(value) = read(&path)
                .ok()
                .and_then(|text| text.parse::<toml::Value>().ok())
            else {
                return false;
            };
            let Some(default) = value
                .get("default_toolchain")
                .and_then(|value| value.as_str())
            else {
                return false;
            };
            pin(pins, family, default, &path);
            if let Some(overrides) = value.get("overrides").and_then(|value| value.as_table()) {
                for (project, toolchain) in overrides {
                    if let Some(toolchain) = toolchain.as_str() {
                        pin(pins, family, toolchain, Path::new(project));
                    } else {
                        return false;
                    }
                }
            }
            if let Ok(value) = std::env::var("RUSTUP_TOOLCHAIN") {
                pin(
                    pins,
                    family,
                    &value,
                    Path::new("RUSTUP_TOOLCHAIN environment"),
                );
            }
            true
        }
        "Node" => {
            let mut alias = "default".to_string();
            for _ in 0..8 {
                let path = home.join(".nvm/alias").join(&alias);
                let Ok(value) = read(&path) else {
                    return false;
                };
                let value = value.trim();
                if value == "system"
                    || value
                        .trim_start_matches('v')
                        .starts_with(|ch: char| ch.is_ascii_digit())
                {
                    pin(pins, family, value, &path);
                    return true;
                }
                if value.is_empty() || value.contains("..") || value.starts_with('/') {
                    return false;
                }
                alias = value.into();
            }
            false
        }
        "Python" => {
            let path = home.join(".pyenv/version");
            let Ok(value) = read(&path) else {
                return false;
            };
            if value.trim().is_empty() {
                return false;
            }
            pin(pins, family, &value, &path);
            if let Ok(value) = std::env::var("PYENV_VERSION") {
                pin(
                    pins,
                    family,
                    &value.replace(':', " "),
                    Path::new("PYENV_VERSION environment"),
                );
            }
            true
        }
        _ => true,
    }
}
fn release_version(value: &str) -> Option<(u64, u64, u64)> {
    let fields: Vec<_> = value.split('.').collect();
    if fields.len() != 3 {
        return None;
    }
    Some((
        fields[0].parse().ok()?,
        fields[1].parse().ok()?,
        fields[2].parse().ok()?,
    ))
}

fn extension_entries(home: &Path, family: &str, relative: &str, size: bool) -> Vec<VersionEntry> {
    let root = home.join(relative);
    let obsolete = read(&root.join(".obsolete"))
        .ok()
        .and_then(|text| serde_json::from_str::<serde_json::Value>(&text).ok());
    let mut registered = std::collections::HashSet::new();
    let mut metadata_ok = true;
    let product = if family == "Cursor" { "Cursor" } else { "Code" };
    let mut indices = vec![root.join("extensions.json")];
    let profiles = home.join(format!(
        "Library/Application Support/{product}/User/profiles"
    ));
    if profiles.exists() {
        match std::fs::read_dir(&profiles) {
            Ok(entries) => {
                for entry in entries {
                    match entry {
                        Ok(entry) if entry.path().is_dir() => {
                            indices.push(entry.path().join("extensions.json"))
                        }
                        Ok(_) => (),
                        Err(_) => metadata_ok = false,
                    }
                }
            }
            Err(_) => metadata_ok = false,
        }
    }
    for index in indices {
        let Some(value) = read(&index)
            .ok()
            .and_then(|text| serde_json::from_str::<serde_json::Value>(&text).ok())
        else {
            metadata_ok = false;
            continue;
        };
        let Some(array) = value.as_array() else {
            metadata_ok = false;
            continue;
        };
        for item in array {
            if let Some(location) = item
                .get("relativeLocation")
                .and_then(|value| value.as_str())
            {
                registered.insert(location.to_string());
            } else if let Some(location) = item
                .get("location")
                .and_then(|value| value.get("path"))
                .and_then(|value| value.as_str())
            {
                if let Some(name) = Path::new(location).file_name() {
                    registered.insert(name.to_string_lossy().into_owned());
                } else {
                    metadata_ok = false;
                }
            } else {
                metadata_ok = false;
            }
        }
    }
    let mut entries = Vec::new();
    if let Ok(children) = std::fs::read_dir(&root) {
        for child in children.flatten() {
            let path = child.path();
            if !path.is_dir() {
                continue;
            }
            let package = read(&path.join("package.json"))
                .ok()
                .and_then(|text| serde_json::from_str::<serde_json::Value>(&text).ok());
            let id = package.as_ref().and_then(|value| {
                Some(format!(
                    "{}.{}",
                    value.get("publisher")?.as_str()?,
                    value.get("name")?.as_str()?
                ))
            });
            let version = package
                .as_ref()
                .and_then(|value| value.get("version"))
                .and_then(|value| value.as_str())
                .unwrap_or("unknown")
                .to_string();
            let mut reasons = vec![];
            let folder = child.file_name().to_string_lossy().into_owned();
            if !metadata_ok {
                reasons.push("Editor registration metadata is missing or unreadable.".into());
            }
            if registered.contains(&folder) {
                reasons.push("Registered in the editor or a profile.".into());
            }
            if obsolete
                .as_ref()
                .and_then(|value| value.get(&folder))
                .and_then(|value| value.as_bool())
                != Some(true)
            {
                reasons.push("Editor has not marked this copy obsolete.".into());
            }
            if id.is_none() || release_version(&version).is_none() {
                reasons.push("Extension identity or release version could not be verified.".into());
            }
            if child.file_type().is_ok_and(|kind| kind.is_symlink()) {
                reasons.push("Linked extensions are protected.".into());
            }
            entries.push(VersionEntry {
                path,
                name: id.unwrap_or(folder),
                family: family.into(),
                version,
                size_bytes: 0,
                removable: false,
                reasons,
            });
        }
    }
    let newer: Vec<_> = entries
        .iter()
        .filter(|entry| {
            registered.contains(
                &entry
                    .path
                    .file_name()
                    .unwrap()
                    .to_string_lossy()
                    .into_owned(),
            )
        })
        .map(|entry| (entry.name.clone(), release_version(&entry.version)))
        .collect();
    for entry in &mut entries {
        if !newer.iter().any(|(id, version)| {
            id == &entry.name
                && version
                    .zip(release_version(&entry.version))
                    .is_some_and(|(new, old)| new > old)
        }) {
            entry.reasons.push(
                "No newer registered copy was verified; keep the only/current version.".into(),
            );
        }
        entry.removable = entry.reasons.is_empty();
        if entry.removable {
            entry
                .reasons
                .push("Obsolete copy; a newer registered version will remain.".into());
        }
        if size {
            entry.size_bytes = crate::core::fs::dir_size(&entry.path);
        }
    }
    entries
}

pub fn inspect(home: &Path, extra: &[PathBuf], size: bool) -> VersionReport {
    let (mut pins, mut report) = gather_pins(home, extra);
    for &(family, relative) in ROOTS {
        if family == "VS Code" || family == "Cursor" {
            let mut entries = extension_entries(home, family, relative, size);
            for entry in &mut entries {
                for pin in pins.iter().filter(|pin| {
                    pin.family == "Extension"
                        && pin.value == format!("{}@{}", entry.name, entry.version)
                }) {
                    entry.removable = false;
                    entry
                        .reasons
                        .push(format!("Project version pin: {}", pin.source));
                }
                if !report.complete {
                    entry.removable = false;
                    entry.reasons.push("Project coverage is incomplete.".into());
                }
            }
            report.entries.extend(entries);
            continue;
        }
        let root = home.join(relative);
        if !root.is_dir() {
            continue;
        }
        let defaults_known = default_pins(home, family, &mut pins);
        let children = match std::fs::read_dir(&root) {
            Ok(children) => children,
            Err(_) => {
                report.warnings.push(format!("Cannot inventory {family}"));
                continue;
            }
        };
        for child in children.flatten() {
            let path = child.path();
            if !path.is_dir() {
                continue;
            }
            let version = child.file_name().to_string_lossy().into_owned();
            let mut reasons = Vec::new();
            let executable = match family {
                "Rust" => "bin/rustc",
                "Node" => "bin/node",
                _ => "bin/python",
            };
            if !path.join(executable).is_file() {
                reasons.push(
                    "Installed runtime executable is missing; identity is unverified.".into(),
                );
            }
            if family == "Rust"
                && !["stable-", "nightly-", "beta-"]
                    .iter()
                    .any(|prefix| version.starts_with(prefix))
                && !version.starts_with(|ch: char| ch.is_ascii_digit())
            {
                reasons.push("Custom toolchains require manual manager review.".into());
            }
            if (family == "Node" && release_version(version.trim_start_matches('v')).is_none())
                || (family == "Python" && release_version(&version).is_none())
            {
                reasons.push("Nonstandard or virtual environment versions are protected.".into());
            }
            if !defaults_known {
                reasons.push("Default or active version could not be verified.".into());
            }
            if !report.complete {
                reasons.push("Project coverage is incomplete.".into());
            }
            for pin in pins
                .iter()
                .filter(|pin| pin.family == family && matches_pin(&version, &pin.value, family))
            {
                reasons.push(format!("Required by {} ({})", pin.source, pin.value));
            }
            if std::env::var_os("PATH").is_some_and(|value| {
                std::env::split_paths(&value).any(|part| part.starts_with(&path))
            }) {
                reasons.push("Active in the current PATH.".into());
            }
            if child.file_type().is_ok_and(|kind| kind.is_symlink()) {
                reasons.push("Linked versions are protected.".into());
            }
            if family == "Rust"
                && ["stable-", "nightly-", "beta-"].iter().any(|prefix| {
                    version.starts_with(prefix)
                        && !version
                            .strip_prefix(prefix)
                            .unwrap()
                            .starts_with(|ch: char| ch.is_ascii_digit())
                })
            {
                reasons.push("Keep the rolling release channel.".into());
            }
            if family == "Python" && path.join("envs").exists() {
                reasons.push("Contains virtual environments.".into());
            }
            let removable = reasons.is_empty();
            if removable {
                reasons.push("No default, active PATH, or project pin matched in the checked roots. Review before removal.".into());
            }
            report.entries.push(VersionEntry {
                name: format!("{family} {version}"),
                family: family.into(),
                version,
                size_bytes: if size {
                    crate::core::fs::dir_size(&path)
                } else {
                    0
                },
                path,
                removable,
                reasons,
            });
        }
    }
    report
        .entries
        .sort_by_key(|entry| std::cmp::Reverse(entry.size_bytes));
    report
}

pub fn validate(path: &Path, projects: &[PathBuf]) -> Result<(), String> {
    let Some(home) = managed_home(path) else {
        return if is_manager_data(path) {
            Err("Select an individual version in Versions Review; manager roots and internal files are protected.".into())
        } else {
            Ok(())
        };
    };
    let report = inspect(&home, projects, false);
    let entry = report
        .entries
        .iter()
        .find(|entry| entry.path == path)
        .ok_or_else(|| "Version could not be verified; refresh Versions Review.".to_string())?;
    if !entry.removable {
        return Err(entry.reasons.join("\n"));
    }
    crate::core::cleanup_targets::validate_open_files(path)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn write(home: &Path, relative: &str, text: &str) {
        let path = home.join(relative);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, text).unwrap();
    }
    fn runtime(home: &Path, family: &str, version: &str) -> PathBuf {
        let relative = ROOTS.iter().find(|(name, _)| *name == family).unwrap().1;
        let path = home.join(relative).join(version);
        let bin = match family {
            "Rust" => "bin/rustc",
            "Node" => "bin/node",
            _ => "bin/python",
        };
        write(&path, bin, "fixture");
        path
    }
    fn removable(report: &VersionReport, path: &Path) -> bool {
        report
            .entries
            .iter()
            .find(|entry| entry.path == path)
            .unwrap()
            .removable
    }
    #[test]
    fn defaults_overrides_project_pins_and_virtualenvs_are_protected() {
        let dir = tempfile::tempdir().unwrap();
        let home = dir.path();
        let stable = runtime(home, "Rust", "stable-aarch64-apple-darwin");
        let pinned = runtime(home, "Rust", "nightly-2025-01-01-aarch64-apple-darwin");
        let old = runtime(home, "Rust", "nightly-2024-01-01-aarch64-apple-darwin");
        let overridden = runtime(home, "Rust", "1.70.0-aarch64-apple-darwin");
        write(home, ".rustup/settings.toml", "default_toolchain='stable-aarch64-apple-darwin'\n[overrides]\n'/external/project'='1.70.0-aarch64-apple-darwin'\n");
        write(
            home,
            "Desktop/project/rust-toolchain.toml",
            "[toolchain]\nchannel='nightly-2025-01-01'\n",
        );
        let node_default = runtime(home, "Node", "v20.19.0");
        let node_pin = runtime(home, "Node", "v18.20.0");
        let node_unused = runtime(home, "Node", "v21.6.2");
        write(home, ".nvm/alias/default", "20\n");
        write(home, "Desktop/project/.nvmrc", "18\n");
        let python_default = runtime(home, "Python", "3.12.0");
        let python_pin = runtime(home, "Python", "3.11.0");
        let python_env = runtime(home, "Python", "3.10.0");
        let python_unused = runtime(home, "Python", "3.9.0");
        write(home, ".pyenv/version", "3.12.0\n");
        write(home, "Desktop/project/.python-version", "3.11.0\n");
        write(&python_env, "envs/myenv/fixture", "data");
        let report = inspect(home, &[], false);
        assert!(report.complete);
        for path in [
            stable,
            pinned,
            overridden,
            node_default,
            node_pin,
            python_default,
            python_pin,
            python_env,
        ] {
            assert!(!removable(&report, &path), "{} must stay", path.display());
        }
        for path in [old, node_unused, python_unused] {
            assert!(removable(&report, &path));
        }
    }
    #[test]
    fn missing_defaults_unreadable_pins_and_external_pins_block_removal() {
        let dir = tempfile::tempdir().unwrap();
        let home = dir.path();
        let node = runtime(home, "Node", "v18.20.0");
        assert!(!removable(&inspect(home, &[], false), &node));
        write(home, ".nvm/alias/default", "system\n");
        write(
            home,
            "Desktop/project/rust-toolchain.toml",
            "broken toml = [",
        );
        let report = inspect(home, &[], false);
        assert!(!report.complete);
        assert!(!removable(&report, &node));
        std::fs::remove_file(home.join("Desktop/project/rust-toolchain.toml")).unwrap();
        let external = tempfile::tempdir().unwrap();
        write(external.path(), ".node-version", "18.20.0\n");
        assert!(!removable(
            &inspect(home, &[external.path().into()], false),
            &node
        ));
        assert!(removable(&inspect(home, &[], false), &node));
        write(
            home,
            "Desktop/project/package.json",
            r#"{"engines":{"node":">=18"}}"#,
        );
        assert!(!removable(&inspect(home, &[], false), &node));
    }
    #[test]
    fn obsolete_extension_requires_newer_registered_copy_and_respects_profiles_and_pins() {
        let dir = tempfile::tempdir().unwrap();
        let home = dir.path();
        for version in ["1.0.0", "2.0.0"] {
            write(
                home,
                &format!(".vscode/extensions/acme.fixture-{version}/package.json"),
                &format!(r#"{{"publisher":"acme","name":"fixture","version":"{version}"}}"#),
            );
        }
        let old = home.join(".vscode/extensions/acme.fixture-1.0.0");
        let new = home.join(".vscode/extensions/acme.fixture-2.0.0");
        write(
            home,
            ".vscode/extensions/.obsolete",
            r#"{"acme.fixture-1.0.0":true}"#,
        );
        write(
            home,
            ".vscode/extensions/extensions.json",
            r#"[{"relativeLocation":"acme.fixture-2.0.0"}]"#,
        );
        let report = inspect(home, &[], false);
        assert!(removable(&report, &old));
        assert!(!removable(&report, &new));
        write(
            home,
            "Library/Application Support/Code/User/profiles/test/extensions.json",
            r#"[{"relativeLocation":"acme.fixture-1.0.0"}]"#,
        );
        assert!(!removable(&inspect(home, &[], false), &old));
        std::fs::remove_dir_all(home.join("Library/Application Support/Code/User/profiles"))
            .unwrap();
        write(
            home,
            "Desktop/project/.vscode/extensions.json",
            r#"{"recommendations":["acme.fixture@1.0.0"]}"#,
        );
        assert!(!removable(&inspect(home, &[], false), &old));
        std::fs::remove_file(home.join("Desktop/project/.vscode/extensions.json")).unwrap();
        write(home, ".vscode/extensions/extensions.json", "malformed");
        assert!(!removable(&inspect(home, &[], false), &old));
    }
    #[test]
    fn jsonc_project_files_work_and_incomplete_version_directives_block_cleanup() {
        assert!(
            jsonc("{ // comment\n \"recommendations\": [\"acme.fixture@1.0.0\",], }").is_some()
        );
        assert!(jsonc("{/* unterminated").is_none());
        let dir = tempfile::tempdir().unwrap();
        let home = dir.path();
        let node = runtime(home, "Node", "v18.20.0");
        write(home, ".nvm/alias/default", "system\n");
        write(home, "Desktop/project/.tool-versions", "nodejs\n");
        let report = inspect(home, &[], false);
        assert!(!report.complete);
        assert!(!removable(&report, &node));
    }

    #[test]
    fn manager_roots_and_internal_files_cannot_bypass_version_review() {
        assert!(managed_home(Path::new("/Users/test/.nvm/versions/node/v20.1.0")).is_some());
        assert!(
            managed_home(Path::new("/Users/test/.nvm/versions/node/v20.1.0/bin/node")).is_none()
        );
        assert!(validate(Path::new("/Users/test/.nvm"), &[]).is_err());
        assert!(validate(Path::new("/Users/test/.vscode/extensions"), &[]).is_err());
        assert!(validate(
            Path::new("/Users/test/.nvm/versions/node/v20.1.0/bin/node"),
            &[]
        )
        .is_err());
    }
}
