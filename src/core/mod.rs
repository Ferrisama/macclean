pub mod cmd;
pub mod fs;

use std::path::PathBuf;

#[derive(Debug, Clone)]
pub struct CleanItem {
    pub label: String,
    pub path: PathBuf,
    pub size_bytes: u64,
    pub removable: bool,
}

#[derive(Debug, Default)]
pub struct AnalysisResult {
    pub items: Vec<CleanItem>,
}

impl AnalysisResult {
    pub fn total_bytes(&self) -> u64 {
        self.items.iter().filter(|i| i.removable).map(|i| i.size_bytes).sum()
    }

    pub fn add(&mut self, label: impl Into<String>, path: PathBuf, size_bytes: u64) {
        self.items.push(CleanItem {
            label: label.into(),
            path,
            size_bytes,
            removable: true,
        });
    }
}

pub trait Cleaner: Send + Sync {
    fn name(&self) -> &str;
    fn display_name(&self) -> &str;
    fn analyze(&self) -> anyhow::Result<AnalysisResult>;
    fn clean(&self, result: &AnalysisResult, dry_run: bool, yes: bool) -> anyhow::Result<()>;
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn total_bytes_sums_removable_only() {
        let mut result = AnalysisResult::default();
        result.items.push(CleanItem {
            label: "a".into(),
            path: PathBuf::from("/tmp/a"),
            size_bytes: 100,
            removable: true,
        });
        result.items.push(CleanItem {
            label: "b".into(),
            path: PathBuf::from("/tmp/b"),
            size_bytes: 200,
            removable: false,
        });
        assert_eq!(result.total_bytes(), 100);
    }

    #[test]
    fn add_creates_removable_item() {
        let mut result = AnalysisResult::default();
        result.add("Trash", PathBuf::from("/tmp"), 1024);
        assert_eq!(result.items.len(), 1);
        assert!(result.items[0].removable);
        assert_eq!(result.items[0].size_bytes, 1024);
    }
}
