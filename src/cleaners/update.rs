use anyhow::Result;
use crate::core::cmd::run_as_user;
use crate::ui::{print_ok, print_warn};

pub fn run(do_brew: bool, do_pip: bool, do_npm: bool) -> Result<()> {
    // ── Homebrew ──────────────────────────────────────────────────────────────
    if do_brew {
        println!("  Updating Homebrew packages...");
        let r = run_as_user(&["brew", "upgrade"]);
        if r.success() {
            print_ok("brew upgrade completed.");
        } else {
            print_warn(&format!(
                "brew upgrade: {}",
                &r.output[..r.output.len().min(300)]
            ));
        }
    }

    // ── pip ───────────────────────────────────────────────────────────────────
    if do_pip {
        println!("  Checking outdated pip packages...");
        let list_r = run_as_user(&["pip", "list", "--outdated", "--format=freeze"]);
        if !list_r.success() {
            print_warn("pip not available or failed to list outdated packages.");
        } else {
            let packages: Vec<&str> = list_r
                .output
                .lines()
                .filter_map(|l| l.split("==").next())
                .filter(|s| !s.trim().is_empty())
                .collect();

            if packages.is_empty() {
                print_ok("All pip packages are up to date.");
            } else {
                println!("  Upgrading {} pip package(s)...", packages.len());
                let mut args = vec!["pip", "install", "--upgrade"];
                args.extend_from_slice(&packages);
                let upgrade_r = run_as_user(&args);
                if upgrade_r.success() {
                    print_ok("pip upgrade completed.");
                } else {
                    print_warn(&format!(
                        "pip upgrade: {}",
                        &upgrade_r.output[..upgrade_r.output.len().min(300)]
                    ));
                }
            }
        }
    }

    // ── npm ───────────────────────────────────────────────────────────────────
    if do_npm {
        println!("  Updating global npm packages...");
        let r = run_as_user(&["npm", "update", "-g"]);
        if r.success() {
            print_ok("npm update -g completed.");
        } else {
            print_warn(&format!(
                "npm update: {}",
                &r.output[..r.output.len().min(300)]
            ));
        }
    }

    Ok(())
}
