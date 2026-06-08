use std::path::PathBuf;
use anyhow::Result;
use crate::core::{AnalysisResult, Cleaner};
use crate::core::fs::dir_size;
use crate::ui;

pub struct XcodeCleaner;

impl Cleaner for XcodeCleaner {
    fn name(&self) -> &str { "xcode" }
    fn display_name(&self) -> &str { "Xcode" }

    fn analyze(&self) -> Result<AnalysisResult> {
        let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("/tmp"));
        let mut result = AnalysisResult::default();

        let xcode_dirs = [
            ("Xcode DerivedData", "Library/Developer/Xcode/DerivedData"),
            ("Xcode Archives", "Library/Developer/Xcode/Archives"),
            ("iOS DeviceSupport", "Library/Developer/Xcode/iOS DeviceSupport"),
            ("watchOS DeviceSupport", "Library/Developer/Xcode/watchOS DeviceSupport"),
            ("tvOS DeviceSupport", "Library/Developer/Xcode/tvOS DeviceSupport"),
            ("visionOS DeviceSupport", "Library/Developer/Xcode/visionOS DeviceSupport"),
            ("CoreSimulator Devices", "Library/Developer/CoreSimulator/Devices"),
            ("CoreSimulator dyld Caches", "Library/Developer/CoreSimulator/Caches/dyld"),
        ];

        for (label, rel) in &xcode_dirs {
            let path = home.join(rel);
            if path.exists() {
                let size = dir_size(&path);
                if size > 0 {
                    result.add(*label, path, size);
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
        if dry_run { return Ok(()); }
        if !yes && !ui::confirm("Clear selected Xcode data?", false)? { return Ok(()); }

        for item in &result.items {
            match std::fs::remove_dir_all(&item.path) {
                Ok(_) => {
                    std::fs::create_dir_all(&item.path).ok();
                    ui::print_ok(&format!("Cleared {}", item.label));
                }
                Err(e) => ui::print_warn(&format!("{}: {}", item.label, e)),
            }
        }
        Ok(())
    }
}
