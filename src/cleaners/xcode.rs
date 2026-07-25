use crate::core::fs::dir_size;
use crate::core::{AnalysisResult, CleanKind, Cleaner, RiskLevel};
use crate::ui;
use anyhow::Result;
use std::path::PathBuf;

pub struct XcodeCleaner;

impl Cleaner for XcodeCleaner {
    fn name(&self) -> &str {
        "xcode"
    }
    fn display_name(&self) -> &str {
        "Xcode"
    }

    fn analyze(&self) -> Result<AnalysisResult> {
        let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("/tmp"));
        let mut result = AnalysisResult::default();

        let xcode_dirs = [
            ("Xcode DerivedData", "Library/Developer/Xcode/DerivedData"),
            ("Xcode Archives", "Library/Developer/Xcode/Archives"),
            (
                "iOS DeviceSupport",
                "Library/Developer/Xcode/iOS DeviceSupport",
            ),
            (
                "watchOS DeviceSupport",
                "Library/Developer/Xcode/watchOS DeviceSupport",
            ),
            (
                "tvOS DeviceSupport",
                "Library/Developer/Xcode/tvOS DeviceSupport",
            ),
            (
                "visionOS DeviceSupport",
                "Library/Developer/Xcode/visionOS DeviceSupport",
            ),
            (
                "CoreDevice device filesystems",
                "Library/Developer/CoreDevice/DeviceFS",
            ),
            (
                "CoreSimulator Devices",
                "Library/Developer/CoreSimulator/Devices",
            ),
            (
                "CoreSimulator dyld Caches",
                "Library/Developer/CoreSimulator/Caches/dyld",
            ),
        ];

        for (label, rel) in &xcode_dirs {
            let path = home.join(rel);
            if path.exists() {
                let size = dir_size(&path);
                if size > 0 {
                    result.add_with_meta(
                        *label,
                        path,
                        size,
                        CleanKind::DevArtifact,
                        RiskLevel::Medium,
                        "Xcode-generated data; can be recreated by Xcode, simulators, or devices.",
                    );
                }
            }
        }

        Ok(result)
    }

    fn clean(&self, result: &AnalysisResult, dry_run: bool, yes: bool) -> Result<()> {
        if result.items.is_empty() {
            println!("No Xcode data found.");
            return Ok(());
        }
        ui::print_analysis("Xcode", &result.items);
        if dry_run {
            return Ok(());
        }
        if !yes && !ui::confirm("Clear selected Xcode data? Simulators and device support files can be recreated by Xcode.", false)? { return Ok(()); }

        for (path, outcome) in crate::core::trash::trash_clean_items("xcode", &result.items) {
            match outcome {
                Ok(_) => {
                    std::fs::create_dir_all(&path).ok();
                    ui::print_ok(&format!("Moved to Trash: {}", path.display()));
                }
                Err(e) => ui::print_warn(&format!("{}: {}", path.display(), e)),
            }
        }
        Ok(())
    }
}
