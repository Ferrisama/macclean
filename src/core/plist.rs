use std::path::Path;

/// Read `CFBundleIdentifier` from an app bundle's `Contents/Info.plist`.
/// Handles both binary (`bplist00`, the default for compiled apps) and XML plists.
pub fn read_bundle_id(app_path: &Path) -> Option<String> {
    let plist_path = app_path.join("Contents/Info.plist");
    let value = plist::Value::from_file(&plist_path).ok()?;
    value
        .as_dictionary()?
        .get("CFBundleIdentifier")?
        .as_string()
        .map(|s| s.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn reads_xml_plist() {
        let dir = tempdir().unwrap();
        let app = dir.path().join("Test.app");
        fs::create_dir_all(app.join("Contents")).unwrap();
        fs::write(
            app.join("Contents/Info.plist"),
            br#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>CFBundleIdentifier</key>
    <string>com.example.Test</string>
</dict>
</plist>"#,
        )
        .unwrap();

        assert_eq!(read_bundle_id(&app), Some("com.example.Test".to_string()));
    }

    #[test]
    fn reads_binary_plist() {
        let dir = tempdir().unwrap();
        let app = dir.path().join("Test.app");
        fs::create_dir_all(app.join("Contents")).unwrap();

        let mut dict = plist::Dictionary::new();
        dict.insert(
            "CFBundleIdentifier".to_string(),
            plist::Value::String("com.example.Binary".to_string()),
        );
        plist::Value::Dictionary(dict)
            .to_file_binary(app.join("Contents/Info.plist"))
            .unwrap();

        assert_eq!(
            read_bundle_id(&app),
            Some("com.example.Binary".to_string())
        );
    }

    #[test]
    fn missing_plist_returns_none() {
        let dir = tempdir().unwrap();
        assert_eq!(read_bundle_id(&dir.path().join("Nope.app")), None);
    }
}
