use std::path::Path;
use walkdir::WalkDir;

pub fn dir_size(path: &Path) -> u64 {
    WalkDir::new(path)
        .follow_links(false)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
        .filter_map(|e| e.metadata().ok())
        .map(|m| m.len())
        .sum()
}

pub fn remove_dir_contents(path: &Path) -> anyhow::Result<()> {
    if !path.exists() {
        return Ok(());
    }
    for entry in std::fs::read_dir(path)? {
        let entry = entry?;
        let p = entry.path();
        if p.is_dir() {
            std::fs::remove_dir_all(&p).ok();
        } else {
            std::fs::remove_file(&p).ok();
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn dir_size_empty() {
        let dir = tempdir().unwrap();
        assert_eq!(dir_size(dir.path()), 0);
    }

    #[test]
    fn dir_size_counts_files() {
        let dir = tempdir().unwrap();
        fs::write(dir.path().join("a.txt"), b"hello").unwrap();
        fs::write(dir.path().join("b.txt"), b"world!").unwrap();
        assert_eq!(dir_size(dir.path()), 11);
    }

    #[test]
    fn remove_dir_contents_clears_files() {
        let dir = tempdir().unwrap();
        fs::write(dir.path().join("file.txt"), b"data").unwrap();
        let sub = dir.path().join("sub");
        fs::create_dir(&sub).unwrap();
        fs::write(sub.join("nested.txt"), b"nested").unwrap();

        remove_dir_contents(dir.path()).unwrap();

        let remaining: Vec<_> = fs::read_dir(dir.path()).unwrap().collect();
        assert!(remaining.is_empty());
    }

    #[test]
    fn remove_dir_contents_nonexistent_is_ok() {
        let result = remove_dir_contents(Path::new("/tmp/macclean_nonexistent_test_dir"));
        assert!(result.is_ok());
    }
}
