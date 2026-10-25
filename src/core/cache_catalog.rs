//! On-demand cache discovery. No maintenance recipes, shell commands, or home walk.
use crate::core::{app_scan::AppScanItem, storage, CleanKind};
use serde::Serialize;
use std::path::{Path, PathBuf};

pub struct CacheRule {
    pub label: &'static str,
    pub relative: &'static str,
    pub consequence: &'static str,
}

impl CacheRule {
    pub fn requires_review(&self) -> bool {
        matches!(
            self.relative,
            ".cargo/registry/src" | ".cargo/git/checkouts"
        )
    }
}

pub const RULES: &[CacheRule] = &[
    CacheRule { label: "npm download cache", relative: ".npm/_cacache", consequence: "Packages must be downloaded again. npm configuration and credentials are excluded." },
    CacheRule { label: "pnpm store", relative: "Library/pnpm/store", consequence: "Dependencies may need to be downloaded again. Quit package installations first." },
    CacheRule { label: "pnpm XDG store", relative: ".local/share/pnpm/store", consequence: "Dependencies may need to be downloaded again. Quit package installations first." },
    CacheRule { label: "Yarn cache", relative: ".yarn/cache", consequence: "Packages must be downloaded again." },
    CacheRule { label: "Yarn Berry cache", relative: ".yarn/berry/cache", consequence: "Packages must be downloaded again." },
    CacheRule { label: "Bun download cache", relative: ".bun/install/cache", consequence: "Packages must be downloaded again. Installed tools are excluded." },
    CacheRule { label: "Cargo registry downloads", relative: ".cargo/registry/cache", consequence: "Crates must be downloaded again. Cargo configuration, credentials, and installed binaries are excluded." },
    CacheRule { label: "Cargo registry sources", relative: ".cargo/registry/src", consequence: "Crate sources are extracted again. Any local changes inside downloaded sources would be lost." },
    CacheRule { label: "Cargo Git checkouts", relative: ".cargo/git/checkouts", consequence: "Git dependencies are fetched again. Any local edits inside these checkouts would be lost." },
    CacheRule { label: "Gradle cache", relative: ".gradle/caches", consequence: "Gradle must regenerate or download cached data. Stop builds first." },
    CacheRule { label: "Gradle distributions", relative: ".gradle/wrapper/dists", consequence: "Gradle distributions must be downloaded again." },
    CacheRule { label: "VS Code cache", relative: "Library/Application Support/Code/Cache", consequence: "Quit VS Code first. Settings, extensions, and sessions are excluded." },
    CacheRule { label: "VS Code compiled cache", relative: "Library/Application Support/Code/CachedData", consequence: "Quit VS Code first. Compiled data will regenerate." },
    CacheRule { label: "VS Code GPU cache", relative: "Library/Application Support/Code/GPUCache", consequence: "Quit VS Code first. GPU cache will regenerate." },
    CacheRule { label: "Codex downloaded runtimes", relative: ".cache/codex-runtimes", consequence: "Quit Codex first. Runtimes must be downloaded again; settings and sessions are excluded." },
    CacheRule { label: "Stremio video cache", relative: "Library/Application Support/stremio-server/stremio-cache", consequence: "Quit Stremio and its server first. Video must stream or download again. Settings and sign-in data are excluded." },
    CacheRule { label: "Stremio server cache", relative: "Library/Application Support/stremio-server/server-cache", consequence: "Quit Stremio and its server first. Cached content regenerates." },
    CacheRule { label: "Stremio desktop cache", relative: "Library/Application Support/Smart Code ltd/Stremio/Cache", consequence: "Quit Stremio first. Settings, WebKit storage, and sign-in data are excluded." },
    CacheRule { label: "Stremio code cache", relative: "Library/Application Support/Smart Code ltd/Stremio/Code Cache", consequence: "Quit Stremio first. Compiled code regenerates." },
    CacheRule { label: "Stremio GPU cache", relative: "Library/Application Support/Smart Code ltd/Stremio/GPUCache", consequence: "Quit Stremio first. GPU data regenerates." },
    CacheRule { label: "Stremio shader cache", relative: "Library/Application Support/Smart Code ltd/Stremio/DawnCache", consequence: "Quit Stremio first. Shader data regenerates." },
];

/// Exact configured roots and descendants, never similarly named neighbors.
pub fn rule_for_path(path: &Path, home: &Path) -> Option<&'static CacheRule> {
    RULES
        .iter()
        .find(|rule| path.starts_with(home.join(rule.relative)))
}

#[derive(Serialize)]
pub struct CacheCatalog {
    pub items: Vec<AppScanItem>,
    pub warnings: Vec<String>,
}

pub fn scan(home: &Path) -> CacheCatalog {
    let mut result = CacheCatalog {
        items: vec![],
        warnings: vec![],
    };
    let home = home.canonicalize().unwrap_or_else(|_| home.to_path_buf());
    let mut roots: Vec<(PathBuf, String, String)> = RULES
        .iter()
        .map(|rule| {
            (
                home.join(rule.relative),
                rule.label.into(),
                rule.consequence.into(),
            )
        })
        .collect();
    let caches = home.join("Library/Caches");
    match std::fs::read_dir(&caches) {
        Ok(entries) => {
            for entry in entries {
                match entry {
                    Ok(entry) => {
                        let name = entry.file_name().to_string_lossy().into_owned();
                        roots.push((entry.path(), format!("{name} cache"), "Quit the owning app before cleanup. This cache may require rebuilding or downloading data; original documents and app settings are outside this location.".into()));
                    }
                    Err(error) => result
                        .warnings
                        .push(format!("{}: {error}", caches.display())),
                }
            }
        }
        Err(error) if error.kind() != std::io::ErrorKind::NotFound => result
            .warnings
            .push(format!("{}: {error}", caches.display())),
        Err(_) => {}
    }
    for (path, label, consequence) in roots {
        let metadata = match path.symlink_metadata() {
            Ok(metadata) => metadata,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
            Err(error) => {
                result.warnings.push(format!("{}: {error}", path.display()));
                continue;
            }
        };
        if metadata.file_type().is_symlink() || !metadata.is_dir() {
            continue;
        }
        if path.canonicalize().ok().as_ref() != Some(&path) {
            result.warnings.push(format!(
                "Skipped symlinked cache location: {}",
                path.display()
            ));
            continue;
        }
        match storage::scan_tree(path.clone(), 0, 1, storage::ScanMode::Deep) {
            Ok(scan) if scan.tree.size_bytes > 0 => {
                let review = rule_for_path(&path, &home).is_some_and(CacheRule::requires_review);
                result.items.push(AppScanItem {
                    name: label,
                    path,
                    size_bytes: scan.tree.size_bytes,
                    logical_size_bytes: scan.tree.logical_size_bytes,
                    hard_link_duplicate: false,
                    percent_of_root: 0.0,
                    is_dir: true,
                    partial: scan.partial,
                    safety: if review {
                        storage::StorageSafety::Review
                    } else {
                        storage::StorageSafety::Safe
                    },
                    clean_kind: CleanKind::Cache,
                    cleanup_action: "Move to Trash".into(),
                    cleanup_reason: consequence,
                });
            }
            Ok(scan) if scan.partial => result
                .warnings
                .push(format!("Could not fully measure {}", path.display())),
            Ok(_) => {}
            Err(error) => result.warnings.push(format!("{}: {error}", path.display())),
        }
    }
    result
        .items
        .sort_by_key(|item| std::cmp::Reverse(item.size_bytes));
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rules_exclude_credentials_settings_and_similar_neighbors() {
        let home = Path::new("/Users/test");
        for relative in [
            ".npm/_cacache/blob",
            ".cargo/registry/cache/crate",
            "Library/pnpm/store/v3",
            "Library/Application Support/stremio-server/stremio-cache/video",
        ] {
            assert!(rule_for_path(&home.join(relative), home).is_some());
        }
        for relative in [
            ".npm",
            ".npmrc",
            ".cargo/credentials.toml",
            ".cargo/bin",
            "Library/pnpm/store-backup",
            "Library/Application Support/Smart Code ltd/Stremio/Local Storage",
            "Library/WebKit/com.westbridge.stremio5-mac",
            "Library/Application Support/stremio-server/stremio-cache-backup",
        ] {
            assert!(
                rule_for_path(&home.join(relative), home).is_none(),
                "{relative}"
            );
        }
        assert!(rule_for_path(Path::new("/Users/other/.npm/_cacache"), home).is_none());
    }
    #[test]
    fn catalog_only_measures_known_roots_and_preserves_coverage() {
        let dir = tempfile::tempdir().unwrap();
        for rel in [
            ".npm/_cacache",
            ".cargo/registry/cache",
            "Library/pnpm/store",
            "Library/Application Support/stremio-server/stremio-cache",
            "Library/Caches/TestApp",
            ".cargo/bin",
            ".npm/settings",
        ] {
            let path = dir.path().join(rel);
            std::fs::create_dir_all(&path).unwrap();
            std::fs::write(path.join("data"), vec![1; 4096]).unwrap();
        }
        let result = scan(dir.path());
        assert_eq!(result.items.len(), 5);
        assert!(result.warnings.is_empty());
        assert!(result.items.iter().all(|item| !item.partial));
        assert!(!result
            .items
            .iter()
            .any(|item| item.path.ends_with("bin") || item.path.ends_with("settings")));
    }
}
