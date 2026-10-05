//! Narrow cleanup rules shared by discovery and the reviewed app Trash flow.
use std::path::{Path, PathBuf};

pub const EDITOR_CACHES: &[&str] = &[
    "Library/Application Support/Code/Cache",
    "Library/Application Support/Code/CachedData",
    "Library/Application Support/Code/CachedExtensionVSIXs",
    "Library/Application Support/Code/Crashpad",
    "Library/Application Support/Code/logs",
    "Library/Application Support/Code/GPUCache",
    ".cache/codex-runtimes",
];

pub fn is_editor_cache(path: &Path) -> bool {
    let components: Vec<_> = path.components().collect();
    EDITOR_CACHES.iter().any(|relative| {
        let pattern: Vec<_> = Path::new(relative).components().collect();
        components
            .windows(pattern.len())
            .any(|parts| parts == pattern)
    })
}

pub fn is_temporary_build(path: &Path) -> bool {
    path.parent() == Some(Path::new("/private/tmp")) || path.parent() == Some(Path::new("/tmp"))
}

pub fn is_rust_build(path: &Path) -> bool {
    let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
        return false;
    };
    let temporary = is_temporary_build(path);
    if name != "target" && !(temporary && name.ends_with("-target")) {
        return false;
    }
    if !path.is_dir() {
        return false;
    }
    let cargo_layout = ["debug", "release"].iter().any(|profile| {
        path.join(profile).join(".fingerprint").is_dir() && path.join(profile).join("deps").is_dir()
    });
    if !cargo_layout {
        return false;
    }
    if temporary {
        return path.join(".rustc_info.json").is_file() && owned_by_current_user(path);
    }
    path.parent()
        .and_then(|parent| std::fs::read_to_string(parent.join("Cargo.toml")).ok())
        .and_then(|manifest| manifest.parse::<toml::Value>().ok())
        .is_some_and(|manifest| {
            manifest.get("package").is_some() || manifest.get("workspace").is_some()
        })
        && owned_by_current_user(path)
}

#[cfg(unix)]
fn owned_by_current_user(path: &Path) -> bool {
    use std::os::unix::fs::MetadataExt;
    path.symlink_metadata().is_ok_and(|metadata| {
        !metadata.file_type().is_symlink() && metadata.uid() == unsafe { libc::getuid() }
    })
}

#[cfg(not(unix))]
fn owned_by_current_user(_path: &Path) -> bool {
    false
}

#[cfg(any(target_os = "macos", test))]
fn open_paths(output: &str, root: &Path) -> bool {
    output
        .lines()
        .filter_map(|line| line.strip_prefix('n'))
        .any(|name| Path::new(name).starts_with(root))
}

/// Run again immediately before moving a newly supported target to Trash.
/// An unavailable activity check is a rejection, never permission to remove.
pub fn validate_idle(path: &Path) -> Result<(), String> {
    if !is_rust_build(path) && !is_editor_cache(path) {
        return Ok(());
    }
    if !owned_by_current_user(path) {
        return Err("Only folders owned by your account can be cleaned here.".into());
    }
    #[cfg(target_os = "macos")]
    {
        let uid = unsafe { libc::getuid() }.to_string();
        if is_rust_build(path) {
            let processes = std::process::Command::new("/bin/ps")
                .args(["-U", &uid, "-o", "comm="])
                .output()
                .map_err(|_| "Cannot check running builds; nothing was removed.".to_string())?;
            if !processes.status.success() {
                return Err("Cannot check running builds; nothing was removed.".into());
            }
            if String::from_utf8_lossy(&processes.stdout)
                .lines()
                .any(|line| {
                    matches!(
                        Path::new(line.trim())
                            .file_name()
                            .and_then(|name| name.to_str()),
                        Some("cargo" | "rustc" | "rustdoc")
                    )
                })
            {
                return Err(
                    "Stop running Rust builds and tests before cleaning build output.".into(),
                );
            }
        }
        validate_open_files(path)
    }
    #[cfg(not(target_os = "macos"))]
    Err("Activity checks for these targets require macOS.".into())
}

pub fn validate_open_files(path: &Path) -> Result<(), String> {
    if !owned_by_current_user(path) {
        return Err("Only folders owned by your account can be cleaned here.".into());
    }
    #[cfg(target_os = "macos")]
    {
        let uid = unsafe { libc::getuid() }.to_string();
        let output = std::process::Command::new("/usr/sbin/lsof")
            .args(["-nP", "-a", "-u", &uid, "-F", "n"])
            .output()
            .map_err(|_| "Cannot check open files; nothing was removed.".to_string())?;
        if !output.status.success() || !output.stderr.is_empty() {
            return Err("Cannot verify open files; close the relevant apps and try again.".into());
        }
        if open_paths(&String::from_utf8_lossy(&output.stdout), path) {
            return Err(
                "This folder is in use. Quit the relevant app or build and review again.".into(),
            );
        }
        Ok(())
    }
    #[cfg(not(target_os = "macos"))]
    Err("Activity checks for these targets require macOS.".into())
}

pub fn editor_cache_paths(home: &Path) -> impl Iterator<Item = PathBuf> + '_ {
    EDITOR_CACHES.iter().map(|relative| home.join(relative))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(target_os = "macos")]
    #[test]
    fn temporary_build_requires_markers_and_does_not_allow_the_tmp_root() {
        let dir = tempfile::Builder::new()
            .prefix("macclean-test-")
            .suffix("-target")
            .tempdir_in("/private/tmp")
            .unwrap();
        assert!(!is_rust_build(dir.path()));
        std::fs::create_dir_all(dir.path().join("debug/.fingerprint")).unwrap();
        std::fs::create_dir_all(dir.path().join("debug/deps")).unwrap();
        assert!(!is_rust_build(dir.path()));
        std::fs::write(dir.path().join(".rustc_info.json"), "{}").unwrap();
        assert!(is_rust_build(dir.path()));
        assert!(!is_rust_build(Path::new("/private/tmp")));
    }

    #[test]
    fn editor_rules_preserve_settings_sessions_and_similarly_named_paths() {
        for path in EDITOR_CACHES {
            assert!(is_editor_cache(&Path::new("/Users/example").join(path)));
        }
        for path in [
            "Library/Application Support/Code/User",
            ".codex/sessions",
            ".cache/codex-runtimes-old",
            "Library/Application Support/Code/CacheBackup",
        ] {
            assert!(!is_editor_cache(&Path::new("/Users/example").join(path)));
        }
    }

    #[test]
    fn rust_output_requires_project_context_and_cargo_layout() {
        let dir = tempfile::tempdir().unwrap();
        let target = dir.path().join("target");
        std::fs::create_dir_all(target.join("debug/.fingerprint")).unwrap();
        std::fs::create_dir_all(target.join("debug/deps")).unwrap();
        assert!(!is_rust_build(&target));
        std::fs::write(
            dir.path().join("Cargo.toml"),
            "[package]\nname='fixture'\nversion='0.1.0'\n",
        )
        .unwrap();
        assert!(is_rust_build(&target));
        std::fs::remove_dir(target.join("debug/.fingerprint")).unwrap();
        assert!(!is_rust_build(&target));
    }

    #[test]
    fn open_file_matching_is_component_bound_and_handles_spaces() {
        let root = Path::new("/private/tmp/test-target");
        assert!(open_paths(
            "p42\nn/private/tmp/test-target/debug/my file\n",
            root
        ));
        assert!(!open_paths(
            "n/private/tmp/test-target-old/debug/file\n",
            root
        ));
    }
}
