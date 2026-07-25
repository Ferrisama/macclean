use crate::core::fs::{dir_size, remove_dir_contents};
use crate::core::{AnalysisResult, Cleaner};
use crate::ui;
use anyhow::Result;
use std::path::PathBuf;

pub struct AndroidCleaner;

impl Cleaner for AndroidCleaner {
    fn name(&self) -> &str {
        "android"
    }
    fn display_name(&self) -> &str {
        "Android SDK Caches"
    }

    fn analyze(&self) -> Result<AnalysisResult> {
        let home = dirs::home_dir().unwrap_or_else(|| PathBuf::from("/tmp"));
        let mut result = AnalysisResult::default();

        let sdk_root = std::env::var_os("ANDROID_HOME")
            .or_else(|| std::env::var_os("ANDROID_SDK_ROOT"))
            .map(PathBuf::from)
            .unwrap_or_else(|| home.join("Library/Android/sdk"));

        let android_dirs = [
            ("Android SDK temporary downloads", sdk_root.join(".temp")),
            (
                "Android SDK emulator system images",
                sdk_root.join("system-images"),
            ),
            ("Android SDK NDK installs", sdk_root.join("ndk")),
            ("Android user cache", home.join(".android/cache")),
            ("Android build cache", home.join(".android/build-cache")),
        ];

        for (label, path) in android_dirs {
            if path.exists() {
                let size = dir_size(&path);
                if size > 0 {
                    result.add(label, path, size);
                }
            }
        }

        Ok(result)
    }

    fn clean(&self, result: &AnalysisResult, dry_run: bool, yes: bool) -> Result<()> {
        if result.items.is_empty() {
            println!("No Android SDK caches found.");
            return Ok(());
        }
        ui::print_analysis("Android SDK Caches", &result.items);
        if dry_run {
            return Ok(());
        }
        if !yes && !ui::confirm("Clear Android SDK caches, emulator system images, and NDK installs? Android Studio can redownload them.", false)? {
            return Ok(());
        }

        for item in &result.items {
            match remove_dir_contents(&item.path) {
                Ok(_) => ui::print_ok(&format!("Cleared {}", item.label)),
                Err(e) => ui::print_warn(&format!("{}: {}", item.label, e)),
            }
        }
        Ok(())
    }
}
