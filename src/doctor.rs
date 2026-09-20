use anyhow::Result;
use colored::Colorize;
use comfy_table::{presets::UTF8_BORDERS_ONLY, Table};
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};

use crate::core::cmd::{is_root, run_cmd};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CheckStatus {
    Ok,
    Warn,
    Fail,
}

impl CheckStatus {
    fn label(self) -> String {
        match self {
            CheckStatus::Ok => "OK".green().to_string(),
            CheckStatus::Warn => "WARN".yellow().to_string(),
            CheckStatus::Fail => "FAIL".red().to_string(),
        }
    }
}

struct DoctorCheck {
    name: &'static str,
    status: CheckStatus,
    detail: String,
    fix: String,
}

pub fn run() -> Result<()> {
    let checks = vec![
        check_platform(),
        check_full_disk_access(),
        check_trash_access(),
        check_app_support_access(),
        check_homebrew(),
        check_docker(),
        check_xcode_tools(),
        check_time_machine(),
        check_codesign_identity(),
    ];

    println!("\n{}", "[ macclean Doctor ]".cyan().bold());
    let mut table = Table::new();
    table.load_preset(UTF8_BORDERS_ONLY);
    table.set_header(vec!["Check", "Status", "Details", "Fix"]);

    for check in &checks {
        table.add_row(vec![
            check.name.to_string(),
            check.status.label(),
            check.detail.clone(),
            check.fix.clone(),
        ]);
    }
    println!("{}", table);

    let failed = checks
        .iter()
        .filter(|check| check.status == CheckStatus::Fail)
        .count();
    let warnings = checks
        .iter()
        .filter(|check| check.status == CheckStatus::Warn)
        .count();

    if failed > 0 {
        crate::ui::print_warn(&format!(
            "{} failing check(s), {} warning(s). Fix failures before deep cleanup.",
            failed, warnings
        ));
    } else if warnings > 0 {
        crate::ui::print_warn(&format!(
            "No hard failures, but {} warning(s) may reduce scan coverage.",
            warnings
        ));
    } else {
        crate::ui::print_ok("All checks passed.");
    }

    Ok(())
}

fn check_platform() -> DoctorCheck {
    if cfg!(target_os = "macos") {
        let version = run_cmd(&["sw_vers", "-productVersion"]).output;
        DoctorCheck {
            name: "macOS",
            status: CheckStatus::Ok,
            detail: if version.is_empty() {
                "macOS detected".into()
            } else {
                format!("macOS {}", version)
            },
            fix: "-".into(),
        }
    } else {
        DoctorCheck {
            name: "macOS",
            status: CheckStatus::Fail,
            detail: "This build is intended for macOS.".into(),
            fix: "Run macclean on macOS.".into(),
        }
    }
}

fn check_full_disk_access() -> DoctorCheck {
    let Some(home) = dirs::home_dir() else {
        return DoctorCheck {
            name: "Full Disk Access",
            status: CheckStatus::Fail,
            detail: "Could not locate home directory.".into(),
            fix: "Check user environment.".into(),
        };
    };

    let probes = [
        home.join("Library/Mail"),
        home.join("Library/Messages"),
        home.join("Library/Safari"),
    ];
    let existing: Vec<_> = probes.iter().filter(|path| path.exists()).collect();
    if existing.is_empty() {
        return DoctorCheck {
            name: "Full Disk Access",
            status: CheckStatus::Warn,
            detail: "No protected probe folders found to verify access.".into(),
            fix: "If scans miss data, grant Full Disk Access to Terminal or the macclean app."
                .into(),
        };
    }

    let denied: Vec<_> = existing
        .iter()
        .filter(|path| fs::read_dir(path).is_err())
        .map(|path| compact_home(path))
        .collect();
    if denied.is_empty() {
        DoctorCheck {
            name: "Full Disk Access",
            status: CheckStatus::Ok,
            detail: "Protected Library probes are readable.".into(),
            fix: "-".into(),
        }
    } else {
        DoctorCheck {
            name: "Full Disk Access",
            status: CheckStatus::Fail,
            detail: format!("Cannot read: {}", denied.join(", ")),
            fix: "System Settings -> Privacy & Security -> Full Disk Access.".into(),
        }
    }
}

fn check_trash_access() -> DoctorCheck {
    let Some(home) = dirs::home_dir() else {
        return DoctorCheck {
            name: "Trash Access",
            status: CheckStatus::Fail,
            detail: "Could not locate ~/.Trash.".into(),
            fix: "Check user environment.".into(),
        };
    };
    let trash = home.join(".Trash");
    if let Err(e) = fs::create_dir_all(&trash) {
        return DoctorCheck {
            name: "Trash Access",
            status: CheckStatus::Fail,
            detail: e.to_string(),
            fix: "Repair ~/.Trash permissions.".into(),
        };
    }

    let test_file = trash.join(format!(".macclean-doctor-{}", std::process::id()));
    let result = OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(&test_file)
        .and_then(|mut file| file.write_all(b"doctor"))
        .and_then(|_| fs::remove_file(&test_file));

    match result {
        Ok(()) => DoctorCheck {
            name: "Trash Access",
            status: CheckStatus::Ok,
            detail: "~/.Trash is writable.".into(),
            fix: "-".into(),
        },
        Err(e) => DoctorCheck {
            name: "Trash Access",
            status: CheckStatus::Fail,
            detail: e.to_string(),
            fix: "Repair ~/.Trash permissions before Trash-backed cleanup.".into(),
        },
    }
}

fn check_app_support_access() -> DoctorCheck {
    let Some(home) = dirs::home_dir() else {
        return DoctorCheck {
            name: "macclean Data",
            status: CheckStatus::Fail,
            detail: "Could not locate home directory.".into(),
            fix: "Check user environment.".into(),
        };
    };
    let dir = home.join("Library/Application Support/macclean");
    match fs::create_dir_all(&dir) {
        Ok(()) => DoctorCheck {
            name: "macclean Data",
            status: CheckStatus::Ok,
            detail: format!("Writable: {}", compact_home(&dir)),
            fix: "-".into(),
        },
        Err(e) => DoctorCheck {
            name: "macclean Data",
            status: CheckStatus::Fail,
            detail: e.to_string(),
            fix: "Repair Application Support permissions.".into(),
        },
    }
}

fn check_homebrew() -> DoctorCheck {
    let brew = run_cmd(&["brew", "--version"]);
    if brew.success() {
        DoctorCheck {
            name: "Homebrew",
            status: CheckStatus::Ok,
            detail: brew
                .output
                .lines()
                .next()
                .unwrap_or("brew found")
                .to_string(),
            fix: "-".into(),
        }
    } else {
        DoctorCheck {
            name: "Homebrew",
            status: CheckStatus::Warn,
            detail: "brew not found.".into(),
            fix: "Install Homebrew or skip brew/update features.".into(),
        }
    }
}

fn check_docker() -> DoctorCheck {
    let docker = run_cmd(&["docker", "info"]);
    if docker.success() {
        DoctorCheck {
            name: "Docker",
            status: CheckStatus::Ok,
            detail: "Docker daemon is reachable.".into(),
            fix: "-".into(),
        }
    } else {
        let version = run_cmd(&["docker", "--version"]);
        if version.success() {
            DoctorCheck {
                name: "Docker",
                status: CheckStatus::Warn,
                detail: "Docker CLI found, daemon not reachable.".into(),
                fix: "Start Docker before running docker cleanup.".into(),
            }
        } else {
            DoctorCheck {
                name: "Docker",
                status: CheckStatus::Warn,
                detail: "docker not found.".into(),
                fix: "Install Docker or skip docker cleanup.".into(),
            }
        }
    }
}

fn check_xcode_tools() -> DoctorCheck {
    let xcode = run_cmd(&["xcode-select", "-p"]);
    if xcode.success() {
        DoctorCheck {
            name: "Xcode Tools",
            status: CheckStatus::Ok,
            detail: xcode.output,
            fix: "-".into(),
        }
    } else {
        DoctorCheck {
            name: "Xcode Tools",
            status: CheckStatus::Warn,
            detail: "xcode-select path unavailable.".into(),
            fix: "Install Command Line Tools if you use Xcode/dev cleanup.".into(),
        }
    }
}

fn check_time_machine() -> DoctorCheck {
    let tmutil = run_cmd(&["tmutil", "listlocalsnapshots", "/"]);
    if tmutil.success() {
        DoctorCheck {
            name: "Time Machine",
            status: CheckStatus::Ok,
            detail: "tmutil local snapshot access works.".into(),
            fix: "-".into(),
        }
    } else {
        DoctorCheck {
            name: "Time Machine",
            status: CheckStatus::Warn,
            detail: "Could not list local snapshots.".into(),
            fix: "Snapshot cleanup may require permissions or no snapshots exist.".into(),
        }
    }
}

fn check_codesign_identity() -> DoctorCheck {
    if is_root() {
        return DoctorCheck {
            name: "Signing Identity",
            status: CheckStatus::Warn,
            detail: "Running as root; user keychain may not be visible.".into(),
            fix: "Run doctor as your normal user before release signing.".into(),
        };
    }

    let identities = run_cmd(&["security", "find-identity", "-p", "codesigning", "-v"]);
    if identities.success() && identities.output.contains("Developer ID Application") {
        DoctorCheck {
            name: "Signing Identity",
            status: CheckStatus::Ok,
            detail: "Developer ID Application identity found.".into(),
            fix: "-".into(),
        }
    } else if identities.success()
        && (identities.output.contains("Apple Development:")
            || identities.output.contains("MacClean Local Development"))
    {
        DoctorCheck {
            name: "Signing Identity",
            status: CheckStatus::Warn,
            detail: "Stable development signing identity found.".into(),
            fix: "Local rebuilds keep their identity; Developer ID is still required for distribution."
                .into(),
        }
    } else {
        DoctorCheck {
            name: "Signing Identity",
            status: CheckStatus::Warn,
            detail: "No Developer ID Application identity detected.".into(),
            fix: "Needed later for signed/notarized releases.".into(),
        }
    }
}

fn compact_home(path: &Path) -> String {
    let path = PathBuf::from(path);
    if let Some(home) = dirs::home_dir() {
        if let Ok(relative) = path.strip_prefix(&home) {
            return format!("~/{}", relative.display());
        }
    }
    path.display().to_string()
}
