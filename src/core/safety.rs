use std::fs;
#[cfg(unix)]
use std::os::unix::fs::MetadataExt;
use std::path::{Component, Path, PathBuf};

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct FileIdentity {
    pub canonical_path: PathBuf,
    pub device: u64,
    pub inode: u64,
    pub is_dir: bool,
    pub is_file: bool,
}

pub fn capture_identity(path: &Path) -> Result<FileIdentity, String> {
    let canonical_path = resolve_existing_path(path)?;
    let metadata = fs::symlink_metadata(&canonical_path)
        .map_err(|error| format!("could not inspect {}: {}", canonical_path.display(), error))?;
    Ok(FileIdentity {
        canonical_path,
        #[cfg(unix)]
        device: metadata.dev(),
        #[cfg(not(unix))]
        device: 0,
        #[cfg(unix)]
        inode: metadata.ino(),
        #[cfg(not(unix))]
        inode: 0,
        is_dir: metadata.file_type().is_dir(),
        is_file: metadata.file_type().is_file(),
    })
}

pub fn validate_identity(path: &Path, expected: &FileIdentity) -> Result<PathBuf, String> {
    let current = capture_identity(path)?;
    if &current != expected {
        return Err(format!(
            "cleanup target changed after review: {}",
            path.display()
        ));
    }
    validate_removal(&current.canonical_path)?;
    Ok(current.canonical_path)
}

const PROTECTED_NAMES: &[&str] = &[".git", ".ssh", ".gnupg"];

pub fn validate_removal(path: &Path) -> Result<(), String> {
    let normalized = resolve_existing_path(path)?;
    if normalized.as_os_str().is_empty() {
        return Err("refusing to remove an empty path".into());
    }
    if normalized == Path::new("/") {
        return Err("refusing to remove filesystem root".into());
    }

    if let Some(home) = dirs::home_dir() {
        let home = normalize(&home);
        if normalized == home {
            return Err("refusing to remove the home directory".into());
        }

        for protected in [
            "Desktop",
            "Documents",
            "Downloads",
            "Library",
            "Applications",
            "Movies",
            "Music",
            "Pictures",
        ] {
            if normalized == home.join(protected) {
                return Err(format!(
                    "refusing to remove protected user folder {}",
                    protected
                ));
            }
        }
    }

    if [
        "/Applications",
        "/Library",
        "/System",
        "/Users",
        "/private",
        "/tmp",
    ]
    .iter()
    .any(|root| normalized == Path::new(root))
    {
        return Err(format!(
            "refusing to remove protected root {}",
            normalized.display()
        ));
    }

    if normalized
        .components()
        .any(|component| protected_component(component))
    {
        return Err("refusing to remove paths inside protected dot-directories".into());
    }

    Ok(())
}

/// Resolves an existing path before any cleanup policy is evaluated. This makes
/// `..` and symlink ancestry visible to the policy instead of classifying the
/// spelling supplied by a caller.
pub fn resolve_existing_path(path: &Path) -> Result<PathBuf, String> {
    let normalized = normalize(path);
    if normalized.as_os_str().is_empty() {
        return Err("refusing to remove an empty path".into());
    }
    fs::canonicalize(&normalized).map_err(|error| {
        format!(
            "could not resolve cleanup path {}: {}",
            normalized.display(),
            error
        )
    })
}

fn protected_component(component: Component<'_>) -> bool {
    let Component::Normal(name) = component else {
        return false;
    };
    name.to_str()
        .is_some_and(|name| PROTECTED_NAMES.contains(&name))
}

fn normalize(path: &Path) -> PathBuf {
    let input = if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir()
            .unwrap_or_else(|_| PathBuf::from("."))
            .join(path)
    };
    let mut normalized = PathBuf::new();
    for component in input.components() {
        match component {
            Component::Prefix(prefix) => normalized.push(prefix.as_os_str()),
            Component::RootDir => normalized.push(Path::new("/")),
            Component::CurDir => {}
            Component::ParentDir => {
                normalized.pop();
            }
            Component::Normal(name) => normalized.push(name),
        }
    }
    normalized
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn rejects_root() {
        assert!(validate_removal(Path::new("/")).is_err());
    }

    #[test]
    fn rejects_git_paths() {
        assert!(validate_removal(Path::new("/tmp/repo/.git/config")).is_err());
    }

    #[test]
    fn resolves_parent_components_before_validation() {
        let resolved = resolve_existing_path(Path::new("/tmp/../tmp")).unwrap();
        assert_eq!(resolved, PathBuf::from("/private/tmp"));
    }

    #[test]
    fn identity_revalidation_rejects_a_replaced_target() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("target");
        fs::write(&path, "original").unwrap();
        let identity = capture_identity(&path).unwrap();
        fs::remove_file(&path).unwrap();
        fs::write(&path, "replacement").unwrap();

        assert!(validate_identity(&path, &identity).is_err());
    }
}
