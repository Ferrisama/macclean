use std::path::{Component, Path, PathBuf};

const PROTECTED_NAMES: &[&str] = &[".git", ".ssh", ".gnupg"];

pub fn validate_removal(path: &Path) -> Result<(), String> {
    let normalized = normalize(path);
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

fn protected_component(component: Component<'_>) -> bool {
    let Component::Normal(name) = component else {
        return false;
    };
    name.to_str()
        .is_some_and(|name| PROTECTED_NAMES.contains(&name))
}

fn normalize(path: &Path) -> PathBuf {
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir()
            .unwrap_or_else(|_| PathBuf::from("."))
            .join(path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_root() {
        assert!(validate_removal(Path::new("/")).is_err());
    }

    #[test]
    fn rejects_git_paths() {
        assert!(validate_removal(Path::new("/tmp/repo/.git/config")).is_err());
    }
}
