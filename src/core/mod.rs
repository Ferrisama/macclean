pub mod cmd;
pub mod fs;
pub mod history;
pub mod plan;
pub mod plist;
pub mod profile;
pub mod safety;
pub mod storage;
pub mod trash;

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CleanKind {
    Cache,
    Log,
    AppTrace,
    DevArtifact,
    Backup,
    Installer,
    Duplicate,
    Maintenance,
    Unknown,
}

impl CleanKind {
    pub fn label(self) -> &'static str {
        match self {
            CleanKind::Cache => "cache",
            CleanKind::Log => "log",
            CleanKind::AppTrace => "app trace",
            CleanKind::DevArtifact => "dev artifact",
            CleanKind::Backup => "backup",
            CleanKind::Installer => "installer",
            CleanKind::Duplicate => "duplicate",
            CleanKind::Maintenance => "maintenance",
            CleanKind::Unknown => "unknown",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, PartialOrd, Ord)]
pub enum RiskLevel {
    Low,
    Medium,
    High,
}

impl RiskLevel {
    pub fn label(self) -> &'static str {
        match self {
            RiskLevel::Low => "low",
            RiskLevel::Medium => "medium",
            RiskLevel::High => "high",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CleanItem {
    pub label: String,
    pub path: PathBuf,
    pub size_bytes: u64,
    pub removable: bool,
    pub kind: CleanKind,
    pub risk: RiskLevel,
    pub reason: String,
}

#[derive(Debug, Default)]
pub struct AnalysisResult {
    pub items: Vec<CleanItem>,
}

impl AnalysisResult {
    pub fn total_bytes(&self) -> u64 {
        self.items
            .iter()
            .filter(|i| i.removable)
            .map(|i| i.size_bytes)
            .sum()
    }

    pub fn add(&mut self, label: impl Into<String>, path: PathBuf, size_bytes: u64) {
        self.add_with_meta(
            label,
            path,
            size_bytes,
            CleanKind::Unknown,
            RiskLevel::Medium,
            "Matched a known macclean cleanup location.",
        );
    }

    pub fn add_with_meta(
        &mut self,
        label: impl Into<String>,
        path: PathBuf,
        size_bytes: u64,
        kind: CleanKind,
        risk: RiskLevel,
        reason: impl Into<String>,
    ) {
        self.items.push(CleanItem {
            label: label.into(),
            path,
            size_bytes,
            removable: true,
            kind,
            risk,
            reason: reason.into(),
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
            kind: CleanKind::Cache,
            risk: RiskLevel::Low,
            reason: "test".into(),
        });
        result.items.push(CleanItem {
            label: "b".into(),
            path: PathBuf::from("/tmp/b"),
            size_bytes: 200,
            removable: false,
            kind: CleanKind::Cache,
            risk: RiskLevel::Low,
            reason: "test".into(),
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
