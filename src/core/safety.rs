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

const REVIEW_TOKEN_PREFIX: &str = "macclean-review-v1:";

/// Encodes a reviewed filesystem identity for transport between the app's
/// dry-run and execution subprocesses. The token is deliberately versioned
/// and opaque to the UI; it is not an authorization credential.
pub fn issue_review_token(identity: &FileIdentity) -> Result<String, String> {
    let payload = serde_json::to_vec(identity)
        .map_err(|error| format!("could not encode cleanup review: {}", error))?;
    let mut encoded = String::with_capacity(REVIEW_TOKEN_PREFIX.len() + payload.len() * 2);
    encoded.push_str(REVIEW_TOKEN_PREFIX);
    for byte in payload {
        use std::fmt::Write as _;
        write!(&mut encoded, "{byte:02x}")
            .map_err(|error| format!("could not encode cleanup review: {}", error))?;
    }
    Ok(encoded)
}

pub fn parse_review_token(token: &str) -> Result<FileIdentity, String> {
    let encoded = token
        .strip_prefix(REVIEW_TOKEN_PREFIX)
        .ok_or_else(|| "invalid or unsupported cleanup review token".to_string())?;
    if encoded.len() % 2 != 0 {
        return Err("invalid cleanup review token".into());
    }
    let mut payload = Vec::with_capacity(encoded.len() / 2);
    for index in (0..encoded.len()).step_by(2) {
        let byte = u8::from_str_radix(&encoded[index..index + 2], 16)
            .map_err(|_| "invalid cleanup review token".to_string())?;
        payload.push(byte);
    }
    serde_json::from_slice(&payload).map_err(|_| "invalid cleanup review token".to_string())
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

    #[test]
    fn review_token_round_trips_identity() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("target");
        fs::write(&path, "original").unwrap();
        let identity = capture_identity(&path).unwrap();

        let token = issue_review_token(&identity).unwrap();

        assert_eq!(parse_review_token(&token).unwrap(), identity);
    }

    #[test]
    fn reviewed_identity_rejects_a_retargeted_symlink() {
        use std::os::unix::fs::symlink;

        let dir = tempdir().unwrap();
        let original = dir.path().join("original");
        let replacement = dir.path().join("replacement");
        let link = dir.path().join("selected");
        fs::write(&original, "original").unwrap();
        fs::write(&replacement, "replacement").unwrap();
        symlink(&original, &link).unwrap();
        let identity = capture_identity(&link).unwrap();
        let token = issue_review_token(&identity).unwrap();

        fs::remove_file(&link).unwrap();
        symlink(&replacement, &link).unwrap();

        let expected = parse_review_token(&token).unwrap();
        assert!(validate_identity(&link, &expected).is_err());
    }

    #[test]
    fn rejects_malformed_review_tokens() {
        assert!(parse_review_token("not-a-review-token").is_err());
        assert!(parse_review_token("macclean-review-v1:0").is_err());
        assert!(parse_review_token("macclean-review-v1:zz").is_err());
    }
}
