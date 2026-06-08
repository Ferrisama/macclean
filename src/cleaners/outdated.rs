use anyhow::Result;
use colored::Colorize;
use comfy_table::{Table, presets::UTF8_BORDERS_ONLY};
use crate::core::cmd::run_as_user;
use crate::ui::{print_ok, print_warn};

pub fn run() -> Result<()> {
    println!("\n{}", "[ Outdated Packages ]".cyan().bold());

    let mut table = Table::new();
    table.load_preset(UTF8_BORDERS_ONLY);
    table.set_header(vec!["Manager", "Package / Info"]);

    let mut any = false;

    // ── Homebrew ──────────────────────────────────────────────────────────────
    let brew_r = run_as_user(&["brew", "outdated", "--verbose"]);
    if brew_r.success() {
        for line in brew_r.output.lines() {
            let trimmed = line.trim();
            if !trimmed.is_empty() {
                table.add_row(vec!["brew".to_string(), trimmed.to_string()]);
                any = true;
            }
        }
    } else {
        print_warn("brew not available or failed.");
    }

    // ── pip ───────────────────────────────────────────────────────────────────
    let pip_r = run_as_user(&["pip", "list", "--outdated", "--format=columns"]);
    if pip_r.success() {
        // Skip first 2 header/separator lines
        for line in pip_r.output.lines().skip(2) {
            let trimmed = line.trim();
            if !trimmed.is_empty() {
                table.add_row(vec!["pip".to_string(), trimmed.to_string()]);
                any = true;
            }
        }
    } else {
        print_warn("pip not available or failed.");
    }

    // ── npm ───────────────────────────────────────────────────────────────────
    let npm_r = run_as_user(&["npm", "outdated", "-g"]);
    // npm exits non-zero when there are outdated packages, so we check output
    for line in npm_r.output.lines().skip(1) {
        let trimmed = line.trim();
        if !trimmed.is_empty() {
            table.add_row(vec!["npm".to_string(), trimmed.to_string()]);
            any = true;
        }
    }

    if any {
        println!("{}", table);
    } else {
        print_ok("All packages are up to date.");
    }

    Ok(())
}
