use clap::{Parser, Subcommand};
use anyhow::Result;
use colored::Colorize;

use crate::cleaners;
use crate::ui;

#[derive(Parser)]
#[command(name = "macclean", about = "Mac system maintenance CLI -- clean, analyze, secure, monitor", version)]
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
    Quick,
    Dev,
    Deep,
    // Cleaners
    Trash,
    System,
    Browser,
    Stremio,
    Apps,
    Xcode,
    Fonts,
    Brew,
    Docker,
    Python,
    Node,
    Pip,
    Cargo,
    Gradle,
    Maven,
    Go,
    Zsh,
    Projects,
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
    Largest {
        #[arg(long, default_value_t = 100u64)]
        min_mb: u64,
        #[arg(long, default_value_t = 30usize)]
        limit: usize,
        #[arg(long)]
        path: Option<std::path::PathBuf>,
    },
    Dupes {
        #[arg(long = "min", default_value_t = 10u64)]
        min_mb: u64,
        #[arg(long)]
        path: Option<std::path::PathBuf>,
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
        app_name: String,
    },
    Outdated,
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

pub fn run() -> Result<()> {
    let cli = Cli::parse();
    let dry_run = cli.dry_run;
    let yes = cli.yes;

    match cli.command {
        None => run_interactive(dry_run, yes),
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
        Commands::Quick => run_preset("quick", dry_run, yes),
        Commands::Dev => run_preset("dev", dry_run, yes),
        Commands::Deep => run_preset("deep", dry_run, yes),

        Commands::Trash => run_cleaner("trash", dry_run, yes),
        Commands::System => run_cleaner("system", dry_run, yes),
        Commands::Browser => run_cleaner("browser", dry_run, yes),
        Commands::Stremio => run_cleaner("stremio", dry_run, yes),
        Commands::Apps => run_cleaner("apps", dry_run, yes),
        Commands::Xcode => run_cleaner("xcode", dry_run, yes),
        Commands::Fonts => run_cleaner("fonts", dry_run, yes),
        Commands::Brew => run_cleaner("brew", dry_run, yes),
        Commands::Docker => run_cleaner("docker", dry_run, yes),
        Commands::Python => run_cleaner("python", dry_run, yes),
        Commands::Node => run_cleaner("node", dry_run, yes),
        Commands::Pip => run_cleaner("pip", dry_run, yes),
        Commands::Cargo => run_cleaner("cargo", dry_run, yes),
        Commands::Gradle => run_cleaner("gradle", dry_run, yes),
        Commands::Maven => run_cleaner("maven", dry_run, yes),
        Commands::Go => run_cleaner("go", dry_run, yes),
        Commands::Zsh => run_cleaner("zsh", dry_run, yes),
        Commands::Projects => run_cleaner("projects", dry_run, yes),
        Commands::Installers => run_cleaner("installers", dry_run, yes),
        Commands::Timemachine => run_cleaner("timemachine", dry_run, yes),
        Commands::Memory => run_cleaner("memory", dry_run, yes),
        Commands::Spotlight => run_cleaner("spotlight", dry_run, yes),
        Commands::Quicklook => run_cleaner("quicklook", dry_run, yes),
        Commands::CrashReports => run_cleaner("crash-reports", dry_run, yes),
        Commands::IosBackups => run_cleaner("ios-backups", dry_run, yes),

        Commands::Health => cleaners::health::run(),
        Commands::Largest { min_mb, limit, path } => cleaners::largest::run(min_mb, limit, path),
        Commands::Dupes { min_mb, path } => cleaners::dupes::run(min_mb, path),
        Commands::Security => cleaners::security::run(),
        Commands::Ports => cleaners::ports::run(),
        Commands::Privacy => cleaners::privacy::run(),
        Commands::Agents => cleaners::agents::run(),
        Commands::LoginItems => cleaners::login_items::run(),
        Commands::Wifi => cleaners::wifi::run(),
        Commands::Connections { process } => cleaners::connections::run(process.as_deref()),
        Commands::Uninstall { app_name } => cleaners::uninstall::run(&app_name, dry_run, yes),
        Commands::Outdated => cleaners::outdated::run(),
        Commands::Update { no_brew, no_pip, no_npm } => {
            cleaners::update::run(!no_brew, !no_pip, !no_npm)
        }
        Commands::QuitApps { configure } => cleaners::quit_apps::run(configure, dry_run, yes),
    }
}

fn run_preset(name: &str, dry_run: bool, yes: bool) -> Result<()> {
    let modules: &[&str] = match name {
        "quick" => &["trash", "browser", "crash-reports"],
        "dev" => &[
            "brew", "docker", "node", "pip", "cargo", "gradle",
            "maven", "go", "xcode", "projects", "zsh",
        ],
        "deep" => &[
            "trash", "system", "browser", "docker", "brew", "xcode",
            "node", "pip", "cargo", "gradle", "maven", "go", "zsh",
            "stremio", "timemachine", "crash-reports", "ios-backups",
            "fonts", "memory", "quicklook", "spotlight", "python",
            "projects", "installers",
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
                    } else if !dry_run {
                        total_bytes += bytes;
                    }
                }
                Err(e) => ui::print_err(&format!("{}: {}", cleaner.display_name(), e)),
            }
        }
    }

    println!("\n{}", format!("Total: {}", ui::format_size(total_bytes)).green().bold());
    Ok(())
}

fn run_category_menu(category: &str, dry_run: bool, yes: bool) -> Result<()> {
    let (label, items): (&str, &[(&str, &str)]) = match category {
        "clean" => (
            "Clean",
            &[
                ("Trash", "trash"),
                ("System Caches & Logs", "system"),
                ("Browser Caches", "browser"),
                ("Docker", "docker"),
                ("Homebrew", "brew"),
                ("Xcode Data", "xcode"),
                ("Node.js Caches", "node"),
                ("pip Cache", "pip"),
                ("Cargo Cache", "cargo"),
                ("Gradle Cache", "gradle"),
                ("Maven Repository", "maven"),
                ("Go Module Cache", "go"),
                ("ZSH History", "zsh"),
                ("Stremio Cache", "stremio"),
                ("Time Machine Snaps", "timemachine"),
                ("Crash Reports", "crash-reports"),
                ("iOS Backups", "ios-backups"),
                ("Duplicate Fonts", "fonts"),
                ("Inactive Memory", "memory"),
                ("QuickLook Cache", "quicklook"),
                ("Spotlight Reindex", "spotlight"),
                ("Python Versions", "python"),
                ("Project Artifacts", "projects"),
                ("Installer Files", "installers"),
            ],
        ),
        "analyze" => (
            "Analyze",
            &[
                ("System Health", "health"),
                ("Largest Files", "largest"),
                ("Duplicate Files", "dupes"),
                ("Outdated Packages", "outdated"),
                ("Wi-Fi Info", "wifi"),
            ],
        ),
        "security" => (
            "Security",
            &[
                ("Security Status", "security"),
                ("App Permissions", "privacy"),
                ("Open Ports", "ports"),
                ("Active Connections", "connections"),
                ("Launch Agents", "agents"),
                ("Login Items", "login-items"),
            ],
        ),
        "manage" => (
            "Manage",
            &[
                ("Update Packages", "update"),
                ("Quit Apps", "quit-apps"),
            ],
        ),
        _ => return Ok(()),
    };

    let display_names: Vec<&str> = items.iter().map(|(name, _)| *name).collect();
    let selected = match inquire::MultiSelect::new(
        &format!("{} -- select commands to run:", label),
        display_names,
    )
    .prompt()
    {
        Ok(s) => s,
        Err(_) => return Ok(()),
    };

    if selected.is_empty() {
        println!("{}", "Nothing selected.".dimmed());
        return Ok(());
    }

    for selected_name in &selected {
        if let Some((_, key)) = items.iter().find(|(n, _)| n == selected_name) {
            println!("\n{}", format!("[ {} ]", selected_name).dimmed());
            match *key {
                "health" => { cleaners::health::run()?; }
                "largest" => { cleaners::largest::run(100, 30, None)?; }
                "dupes" => { cleaners::dupes::run(10, None)?; }
                "outdated" => { cleaners::outdated::run()?; }
                "wifi" => { cleaners::wifi::run()?; }
                "security" => { cleaners::security::run()?; }
                "privacy" => { cleaners::privacy::run()?; }
                "ports" => { cleaners::ports::run()?; }
                "connections" => { cleaners::connections::run(None)?; }
                "agents" => { cleaners::agents::run()?; }
                "login-items" => { cleaners::login_items::run()?; }
                "update" => { cleaners::update::run(true, true, true)?; }
                "quit-apps" => { cleaners::quit_apps::run(false, dry_run, yes)?; }
                name => { run_cleaner(name, dry_run, yes)?; }
            }
        }
    }
    Ok(())
}

fn run_interactive(dry_run: bool, yes: bool) -> Result<()> {
    println!("{}", "macclean -- Mac system maintenance".cyan().bold());
    println!("{}", "Arrow keys  Enter to select  Ctrl+C to quit".dimmed());
    println!();

    let top_options = vec![
        "Quick Clean   (trash + browser + crash reports)",
        "Dev Clean     (brew + docker + node/pip/cargo + xcode + projects + zsh)",
        "Deep Clean    (everything)",
        "  Clean  ->",
        "  Analyze ->",
        "  Security ->",
        "  Manage  ->",
    ];

    let choice = match inquire::Select::new("What would you like to do?", top_options).prompt() {
        Ok(c) => c,
        Err(_) => return Ok(()),
    };

    match choice {
        c if c.starts_with("Quick") => run_preset("quick", dry_run, yes),
        c if c.starts_with("Dev") => run_preset("dev", dry_run, yes),
        c if c.starts_with("Deep") => run_preset("deep", dry_run, yes),
        c if c.contains("Clean") => run_category_menu("clean", dry_run, yes),
        c if c.contains("Analyze") => run_category_menu("analyze", dry_run, yes),
        c if c.contains("Security") => run_category_menu("security", dry_run, yes),
        c if c.contains("Manage") => run_category_menu("manage", dry_run, yes),
        _ => Ok(()),
    }
}
