use anyhow::Result;
use colored::Colorize;
use std::collections::HashSet;
use std::path::PathBuf;
use crate::core::cmd::run_cmd;
use crate::ui::{confirm, print_ok, print_warn};

const ALWAYS_KEEP: &[&str] = &["Finder"];

const DEFAULT_KEEP: &[&str] = &[
    "Finder",
    "Google Chrome",
    "Safari",
    "Firefox",
    "Brave Browser",
    "Arc",
    "Visual Studio Code",
    "Cursor",
    "iTerm2",
    "Terminal",
    "Warp",
];

fn config_path() -> PathBuf {
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".macclean_quit_apps.json")
}

fn load_keep_list() -> Option<Vec<String>> {
    let path = config_path();
    if !path.exists() {
        return None;
    }
    let text = std::fs::read_to_string(&path).ok()?;
    // Quick sanity check: must contain at least one quote
    if !text.contains('"') { return None; }
    // Minimal JSON parse: find array content after "keep":
    let keep_pos = text.find("\"keep\"")?;
    let rest     = &text[keep_pos + 6..];
    let arr_start = rest.find('[')? + 1;
    let arr_end   = rest.find(']')?;
    let arr_content = &rest[arr_start..arr_end];

    let items: Vec<String> = arr_content
        .split(',')
        .filter_map(|s| {
            let trimmed = s.trim().trim_matches('"').trim().to_string();
            if trimmed.is_empty() { None } else { Some(trimmed) }
        })
        .collect();

    Some(items)
}

fn save_keep_list(keep: &[String]) -> anyhow::Result<()> {
    let entries: Vec<String> = keep
        .iter()
        .map(|s| format!("  \"{}\"", s))
        .collect();
    let json = format!("{{\n  \"keep\": [\n{}\n  ]\n}}\n", entries.join(",\n"));
    std::fs::write(config_path(), json)?;
    Ok(())
}

fn get_running_apps() -> Vec<String> {
    let r = run_cmd(&[
        "osascript",
        "-e",
        "tell application \"System Events\" to get name of every process where background only is false",
    ]);
    if !r.success() || r.output.trim().is_empty() {
        return Vec::new();
    }
    r.output
        .split(',')
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect()
}

pub fn run(do_configure: bool, dry_run: bool, yes: bool) -> Result<()> {
    println!("\n{}", "[ Quit Apps ]".cyan().bold());

    let running = get_running_apps();
    if running.is_empty() {
        print_warn("Could not retrieve running applications (System Events access required).");
        return Ok(());
    }

    println!("  Running apps: {}", running.len());

    // ── Determine keep list ───────────────────────────────────────────────────
    let existing_config = load_keep_list();
    let needs_configure  = do_configure || existing_config.is_none();

    let keep_list: Vec<String> = if needs_configure {
        // Interactive selection with inquire
        use inquire::MultiSelect;

        let options: Vec<String> = running.clone();
        let defaults: Vec<usize> = options
            .iter()
            .enumerate()
            .filter(|(_, a)| DEFAULT_KEEP.contains(&a.as_str()))
            .map(|(i, _)| i)
            .collect();

        let selected = MultiSelect::new("Keep these apps open:", options)
            .with_default(&defaults)
            .prompt()?;

        // Always ensure ALWAYS_KEEP entries are in the list
        let mut keep = selected;
        for app in ALWAYS_KEEP {
            if !keep.iter().any(|k| k == app) {
                keep.push(app.to_string());
            }
        }

        save_keep_list(&keep)?;
        println!("  Saved keep list to {}", config_path().display());
        keep
    } else {
        let mut list = existing_config.unwrap_or_default();
        // Ensure ALWAYS_KEEP
        for app in ALWAYS_KEEP {
            if !list.iter().any(|k| k == app) {
                list.push(app.to_string());
            }
        }
        list
    };

    let keep_set: HashSet<&str> = keep_list.iter().map(|s| s.as_str()).collect();

    let to_quit: Vec<&str> = running
        .iter()
        .map(|s| s.as_str())
        .filter(|app| !keep_set.contains(*app))
        .collect();

    if to_quit.is_empty() {
        println!("  Nothing to quit.");
        return Ok(());
    }

    println!("  Apps to quit: {}", to_quit.join(", "));

    if dry_run {
        print_warn("Dry run — no apps quit.");
        return Ok(());
    }

    if !yes && !confirm(&format!("Quit {} app(s)?", to_quit.len()), false)? {
        println!("  Aborted.");
        return Ok(());
    }

    for app in &to_quit {
        let r = run_cmd(&[
            "osascript",
            "-e",
            &format!("tell application \"{}\" to quit", app),
        ]);
        if r.success() {
            print_ok(&format!("Quit: {}", app));
        } else {
            print_warn(&format!("Could not quit {}: {}", app, &r.output[..r.output.len().min(120)]));
        }
    }

    Ok(())
}
