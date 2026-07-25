use anyhow::Result;
use clap::{Parser, Subcommand};
use colored::Colorize;

use crate::cleaners;
use crate::ui;

#[derive(Parser)]
#[command(
    name = "macclean",
    about = "Mac system maintenance CLI -- clean, analyze, secure, monitor",
    version
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<Commands>,

    #[arg(short = 'n', long, global = true, help = "Analyze only, no deletion")]
    pub dry_run: bool,

    #[arg(short = 'y', long, global = true, help = "Skip confirmation prompts")]
    pub yes: bool,
}

#[derive(Subcommand)]
pub enum Commands {
    // Presets
    Ask {
        #[arg(required = true)]
        query: Vec<String>,
    },
    Quick,
    Dev,
    Deep,
    // Cleaners
    Trash,
    System,
    #[command(name = "system-data")]
    SystemData {
        #[arg(long)]
        path: Option<std::path::PathBuf>,
        #[arg(long, default_value_t = 1usize)]
        depth: usize,
        #[arg(long, default_value_t = 8usize)]
        limit: usize,
        #[arg(long)]
        json: bool,
        #[arg(long)]
        deep: bool,
    },
    Browser,
    Stremio,
    Apps,
    Xcode,
    Fonts,
    Brew,
    Docker,
    Android,
    Python,
    Node,
    Pip,
    Cargo,
    Gradle,
    Maven,
    Go,
    Zsh,
    Projects {
        #[arg(long)]
        path: Option<std::path::PathBuf>,
        #[arg(long, value_delimiter = ',')]
        only: Vec<String>,
        #[arg(long)]
        exclude: Vec<std::path::PathBuf>,
        #[arg(long = "older-than-days")]
        older_than_days: Option<u64>,
        #[arg(long, default_value_t = 5usize)]
        depth: usize,
        #[arg(long = "save-plan")]
        save_plan: Option<String>,
    },
    Installers,
    Timemachine,
    Memory,
    Spotlight,
    Quicklook,
    #[command(name = "crash-reports")]
    CrashReports,
    #[command(name = "ios-backups")]
    IosBackups,
    // Tools
    Health,
    Doctor,
    Scan {
        #[arg(default_value = ".")]
        path: std::path::PathBuf,
        #[arg(long, default_value_t = 1usize)]
        depth: usize,
        #[arg(long, default_value_t = 12usize)]
        limit: usize,
        #[arg(long)]
        json: bool,
        #[arg(long)]
        deep: bool,
    },
    Largest {
        #[arg(long, default_value_t = 100u64)]
        min_mb: u64,
        #[arg(long, default_value_t = 30usize)]
        limit: usize,
        #[arg(long)]
        path: Option<std::path::PathBuf>,
        #[arg(long)]
        trash: bool,
    },
    Dupes {
        #[arg(long = "min", default_value_t = 10u64)]
        min_mb: u64,
        #[arg(long)]
        path: Option<std::path::PathBuf>,
        #[arg(long)]
        trash: bool,
        #[arg(long, default_value = "first", value_parser = ["first", "newest", "oldest", "shortest", "shortest-path"])]
        keep: String,
    },
    Security,
    Ports,
    Privacy,
    Agents,
    #[command(name = "login-items")]
    LoginItems,
    Wifi,
    Connections {
        #[arg(long)]
        process: Option<String>,
    },
    Uninstall {
        app_name: Option<String>,
        #[arg(long)]
        path: Option<std::path::PathBuf>,
        #[arg(long = "bundle-id")]
        bundle_id: Option<String>,
        #[arg(long)]
        deep: bool,
    },
    Outdated,
    History {
        #[arg(long, default_value_t = 20usize)]
        limit: usize,
    },
    Restore {
        session: Option<String>,
    },
    Plan {
        #[command(subcommand)]
        command: PlanCommand,
    },
    Profile {
        #[command(subcommand)]
        command: ProfileCommand,
    },
    Update {
        #[arg(long)]
        no_brew: bool,
        #[arg(long)]
        no_pip: bool,
        #[arg(long)]
        no_npm: bool,
    },
    #[command(name = "quit-apps")]
    QuitApps {
        #[arg(long)]
        configure: bool,
    },
}

#[derive(Subcommand)]
pub enum PlanCommand {
    Create {
        name: String,
        #[arg(required = true)]
        paths: Vec<std::path::PathBuf>,
    },
    List,
    Show {
        name: String,
    },
    Apply {
        name: String,
    },
    Remove {
        name: String,
    },
}

#[derive(Subcommand)]
pub enum ProfileCommand {
    #[command(name = "create-project")]
    CreateProject {
        name: String,
        #[arg(long)]
        path: Option<std::path::PathBuf>,
        #[arg(long, value_delimiter = ',')]
        only: Vec<String>,
        #[arg(long)]
        exclude: Vec<std::path::PathBuf>,
        #[arg(long = "older-than-days")]
        older_than_days: Option<u64>,
        #[arg(long, default_value_t = 5usize)]
        depth: usize,
        #[arg(long = "risk-ceiling", default_value = "medium", value_parser = ["low", "medium", "high"])]
        risk_ceiling: String,
    },
    List,
    Run {
        name: String,
    },
    Remove {
        name: String,
    },
}

pub fn run() -> Result<()> {
    let cli = Cli::parse();
    let dry_run = cli.dry_run;
    let yes = cli.yes;

    match cli.command {
        None => crate::tui::run(dry_run, yes),
        Some(cmd) => dispatch(cmd, dry_run, yes),
    }
}

fn run_cleaner(name: &str, dry_run: bool, yes: bool) -> Result<()> {
    let Some(cleaner) = cleaners::cleaner_by_name(name) else {
        eprintln!("Unknown cleaner: {}", name);
        return Ok(());
    };
    match cleaner.analyze() {
        Ok(result) => {
            if let Err(e) = cleaner.clean(&result, dry_run, yes) {
                ui::print_warn(&format!("{}: {}", cleaner.display_name(), e));
            }
        }
        Err(e) => ui::print_err(&format!("Error in {}: {}", cleaner.display_name(), e)),
    }
    Ok(())
}

fn dispatch(cmd: Commands, dry_run: bool, yes: bool) -> Result<()> {
    match cmd {
        Commands::Ask { query } => crate::ask::run(&query, dry_run, yes),
        Commands::Quick => run_preset("quick", dry_run, yes),
        Commands::Dev => run_preset("dev", dry_run, yes),
        Commands::Deep => run_preset("deep", dry_run, yes),

        Commands::Trash => run_cleaner("trash", dry_run, yes),
        Commands::System => run_cleaner("system", dry_run, yes),
        Commands::SystemData {
            path,
            depth,
            limit,
            json,
            deep,
        } => cleaners::system_data::run(path, depth, limit, scan_mode(deep), json),
        Commands::Browser => run_cleaner("browser", dry_run, yes),
        Commands::Stremio => run_cleaner("stremio", dry_run, yes),
        Commands::Apps => run_cleaner("apps", dry_run, yes),
        Commands::Xcode => run_cleaner("xcode", dry_run, yes),
        Commands::Fonts => run_cleaner("fonts", dry_run, yes),
        Commands::Brew => run_cleaner("brew", dry_run, yes),
        Commands::Docker => run_cleaner("docker", dry_run, yes),
        Commands::Android => run_cleaner("android", dry_run, yes),
        Commands::Python => run_cleaner("python", dry_run, yes),
        Commands::Node => run_cleaner("node", dry_run, yes),
        Commands::Pip => run_cleaner("pip", dry_run, yes),
        Commands::Cargo => run_cleaner("cargo", dry_run, yes),
        Commands::Gradle => run_cleaner("gradle", dry_run, yes),
        Commands::Maven => run_cleaner("maven", dry_run, yes),
        Commands::Go => run_cleaner("go", dry_run, yes),
        Commands::Zsh => run_cleaner("zsh", dry_run, yes),
        Commands::Projects {
            path,
            only,
            exclude,
            older_than_days,
            depth,
            save_plan,
        } => {
            let options = cleaners::projects::ProjectScanOptions {
                root: path.unwrap_or_else(|| {
                    dirs::home_dir().unwrap_or_else(|| std::path::PathBuf::from("/tmp"))
                }),
                max_depth: depth,
                only,
                exclude,
                older_than_days,
            };
            if let Some(plan_name) = save_plan {
                let result = cleaners::projects::analyze_with_options(&options)?;
                let plan = crate::core::plan::create_from_clean_items(
                    &plan_name,
                    "projects",
                    &result.items,
                )?;
                let path = crate::core::plan::save(&plan)?;
                ui::print_plan(&plan);
                ui::print_ok(&format!("Saved plan '{}': {}", plan_name, path.display()));
                Ok(())
            } else {
                cleaners::projects::run(options, dry_run, yes)
            }
        }
        Commands::Installers => run_cleaner("installers", dry_run, yes),
        Commands::Timemachine => run_cleaner("timemachine", dry_run, yes),
        Commands::Memory => run_cleaner("memory", dry_run, yes),
        Commands::Spotlight => run_cleaner("spotlight", dry_run, yes),
        Commands::Quicklook => run_cleaner("quicklook", dry_run, yes),
        Commands::CrashReports => run_cleaner("crash-reports", dry_run, yes),
        Commands::IosBackups => run_cleaner("ios-backups", dry_run, yes),

        Commands::Health => cleaners::health::run(),
        Commands::Doctor => crate::doctor::run(),
        Commands::Scan {
            path,
            depth,
            limit,
            json,
            deep,
        } => run_storage_scan(path, depth, limit, json, deep),
        Commands::Largest {
            min_mb,
            limit,
            path,
            trash,
        } => cleaners::largest::run(min_mb, limit, path, trash, dry_run, yes),
        Commands::Dupes {
            min_mb,
            path,
            trash,
            keep,
        } => cleaners::dupes::run(min_mb, path, trash, &keep, dry_run, yes),
        Commands::Security => cleaners::security::run(),
        Commands::Ports => cleaners::ports::run(),
        Commands::Privacy => cleaners::privacy::run(),
        Commands::Agents => cleaners::agents::run(),
        Commands::LoginItems => cleaners::login_items::run(),
        Commands::Wifi => cleaners::wifi::run(),
        Commands::Connections { process } => cleaners::connections::run(process.as_deref()),
        Commands::Uninstall {
            app_name,
            path,
            bundle_id,
            deep,
        } => cleaners::uninstall::run_with_options(
            cleaners::uninstall::UninstallOptions {
                app_name,
                app_path: path,
                bundle_id,
                deep,
            },
            dry_run,
            yes,
        ),
        Commands::Outdated => cleaners::outdated::run(),
        Commands::History { limit } => ui::print_history(limit),
        Commands::Restore { session } => {
            let outcomes = crate::core::history::restore_session_detailed(session.as_deref())?;
            ui::print_restore_outcomes(&outcomes);
            let restored = outcomes.iter().filter(|outcome| outcome.restored).count();
            let failed = outcomes.len().saturating_sub(restored);
            ui::print_ok(&format!("Restored {} item(s)", restored));
            if failed > 0 {
                ui::print_warn(&format!(
                    "{} item(s) could not be restored automatically. Check macclean history and the Trash.",
                    failed
                ));
            }
            Ok(())
        }
        Commands::Plan { command } => dispatch_plan(command, dry_run, yes),
        Commands::Profile { command } => dispatch_profile(command, dry_run, yes),
        Commands::Update {
            no_brew,
            no_pip,
            no_npm,
        } => cleaners::update::run(!no_brew, !no_pip, !no_npm),
        Commands::QuitApps { configure } => cleaners::quit_apps::run(configure, dry_run, yes),
    }
}

fn dispatch_plan(command: PlanCommand, dry_run: bool, yes: bool) -> Result<()> {
    match command {
        PlanCommand::Create { name, paths } => {
            let plan = crate::core::plan::create_from_paths(&name, &paths)?;
            let path = crate::core::plan::save(&plan)?;
            ui::print_ok(&format!(
                "Saved plan '{}' with {} item(s), {} total: {}",
                plan.name,
                plan.items.len(),
                ui::format_size(plan.total_bytes()),
                path.display()
            ));
            Ok(())
        }
        PlanCommand::List => {
            let plans = crate::core::plan::list()?;
            if plans.is_empty() {
                println!("No saved plans.");
            } else {
                for name in plans {
                    println!("{}", name);
                }
            }
            Ok(())
        }
        PlanCommand::Show { name } => {
            let plan = crate::core::plan::load(&name)?;
            ui::print_plan(&plan);
            Ok(())
        }
        PlanCommand::Apply { name } => {
            let plan = crate::core::plan::load(&name)?;
            ui::print_plan(&plan);
            if dry_run {
                crate::core::plan::validate_plan(&plan)?;
                ui::print_warn("Dry run -- plan validated, nothing moved to Trash.");
                return Ok(());
            }
            if !yes
                && !ui::confirm(
                    &format!(
                        "Apply plan '{}' and move {} item(s) to Trash?",
                        name,
                        plan.items.len()
                    ),
                    false,
                )?
            {
                return Ok(());
            }
            for (path, outcome) in crate::core::plan::apply(&name, false)? {
                match outcome {
                    Ok(_) => ui::print_ok(&format!("Moved to Trash: {}", path.display())),
                    Err(e) => ui::print_warn(&format!("{}: {}", path.display(), e)),
                }
            }
            Ok(())
        }
        PlanCommand::Remove { name } => {
            crate::core::plan::remove(&name)?;
            ui::print_ok(&format!("Removed plan '{}'", name));
            Ok(())
        }
    }
}

fn dispatch_profile(command: ProfileCommand, dry_run: bool, yes: bool) -> Result<()> {
    match command {
        ProfileCommand::CreateProject {
            name,
            path,
            only,
            exclude,
            older_than_days,
            depth,
            risk_ceiling,
        } => {
            let options = cleaners::projects::ProjectScanOptions {
                root: path.unwrap_or_else(|| {
                    dirs::home_dir().unwrap_or_else(|| std::path::PathBuf::from("/tmp"))
                }),
                max_depth: depth,
                only,
                exclude,
                older_than_days,
            };
            let path = crate::core::profile::create_project_profile(
                &name,
                options,
                parse_risk(&risk_ceiling),
            )?;
            ui::print_ok(&format!(
                "Saved project profile '{}': {}",
                name,
                path.display()
            ));
            Ok(())
        }
        ProfileCommand::List => {
            let profiles = crate::core::profile::list()?;
            if profiles.is_empty() {
                println!("No saved profiles.");
            } else {
                for name in profiles {
                    println!("{}", name);
                }
            }
            Ok(())
        }
        ProfileCommand::Run { name } => {
            let profile = crate::core::profile::load_project_profile(&name)?;
            cleaners::projects::run_with_risk_ceiling(
                profile.options,
                profile.risk_ceiling,
                dry_run,
                yes,
            )
        }
        ProfileCommand::Remove { name } => {
            crate::core::profile::remove(&name)?;
            ui::print_ok(&format!("Removed profile '{}'", name));
            Ok(())
        }
    }
}

fn parse_risk(value: &str) -> crate::core::RiskLevel {
    match value {
        "low" => crate::core::RiskLevel::Low,
        "high" => crate::core::RiskLevel::High,
        _ => crate::core::RiskLevel::Medium,
    }
}

fn scan_mode(deep: bool) -> crate::core::storage::ScanMode {
    if deep {
        crate::core::storage::ScanMode::Deep
    } else {
        crate::core::storage::ScanMode::Fast
    }
}

fn run_storage_scan(
    path: std::path::PathBuf,
    depth: usize,
    limit: usize,
    json: bool,
    deep: bool,
) -> Result<()> {
    if json {
        let scan = crate::core::storage::scan_tree(path, depth, limit, scan_mode(deep))?;
        let _ = crate::core::storage::write_tree_cache(&scan);
        println!("{}", serde_json::to_string_pretty(&scan)?);
    } else {
        cleaners::system_data::run(Some(path), depth, limit, scan_mode(deep), false)?;
    }
    Ok(())
}

fn run_preset(name: &str, dry_run: bool, yes: bool) -> Result<()> {
    let modules: &[&str] = match name {
        "quick" => &["trash", "browser", "crash-reports"],
        "dev" => &[
            "brew", "docker", "node", "pip", "cargo", "gradle", "android", "maven", "go", "xcode",
            "projects", "zsh",
        ],
        "deep" => &[
            "trash",
            "system",
            "browser",
            "docker",
            "brew",
            "xcode",
            "node",
            "pip",
            "cargo",
            "gradle",
            "android",
            "maven",
            "go",
            "zsh",
            "stremio",
            "timemachine",
            "crash-reports",
            "ios-backups",
            "fonts",
            "memory",
            "quicklook",
            "spotlight",
            "python",
            "projects",
            "installers",
        ],
        _ => return Ok(()),
    };

    let label = match name {
        "quick" => "Quick Clean",
        "dev" => "Dev Clean",
        "deep" => "Deep Clean",
        _ => name,
    };

    println!("\n{}", format!("-- {} --", label).cyan().bold());

    let mut total_bytes: u64 = 0;
    for module in modules {
        if let Some(cleaner) = cleaners::cleaner_by_name(module) {
            println!("\n{}", format!("[ {} ]", cleaner.display_name()).dimmed());
            match cleaner.analyze() {
                Ok(result) => {
                    let bytes = result.total_bytes();
                    if let Err(e) = cleaner.clean(&result, dry_run, yes) {
                        ui::print_warn(&format!("{}: {}", cleaner.display_name(), e));
                    } else {
                        total_bytes += bytes;
                    }
                }
                Err(e) => ui::print_err(&format!("{}: {}", cleaner.display_name(), e)),
            }
        }
    }

    let total_label = if dry_run {
        "Total recoverable"
    } else {
        "Total"
    };
    println!(
        "\n{}",
        format!("{}: {}", total_label, ui::format_size(total_bytes))
            .green()
            .bold()
    );
    Ok(())
}
