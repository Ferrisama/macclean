pub mod agents;
pub mod apps;
pub mod brew;
pub mod browser;
pub mod cargo_cache;
pub mod connections;
pub mod crash_reports;
pub mod docker;
pub mod dupes;
pub mod fonts;
pub mod go_cache;
pub mod gradle;
pub mod health;
pub mod installers;
pub mod ios_backups;
pub mod largest;
pub mod login_items;
pub mod maven;
pub mod memory;
pub mod node;
pub mod outdated;
pub mod pip;
pub mod ports;
pub mod privacy;
pub mod projects;
pub mod python_versions;
pub mod quicklook;
pub mod quit_apps;
pub mod security;
pub mod spotlight;
pub mod stremio;
pub mod system;
pub mod timemachine;
pub mod trash;
pub mod uninstall;
pub mod update;
pub mod wifi;
pub mod xcode;
pub mod zsh;

use crate::core::Cleaner;

pub fn all_cleaners() -> Vec<Box<dyn Cleaner>> {
    vec![
        Box::new(trash::TrashCleaner),
        Box::new(system::SystemCleaner),
        Box::new(browser::BrowserCleaner),
        Box::new(node::NodeCleaner),
        Box::new(pip::PipCleaner),
        Box::new(cargo_cache::CargoCacheCleaner),
        Box::new(gradle::GradleCleaner),
        Box::new(maven::MavenCleaner),
        Box::new(go_cache::GoCacheCleaner),
        Box::new(crash_reports::CrashReportsCleaner),
        Box::new(quicklook::QuickLookCleaner),
        Box::new(stremio::StremioCleaner),
        Box::new(installers::InstallersCleaner),
        Box::new(brew::BrewCleaner),
        Box::new(docker::DockerCleaner),
        Box::new(xcode::XcodeCleaner),
        Box::new(timemachine::TimemachineCleaner),
        Box::new(memory::MemoryCleaner),
        Box::new(spotlight::SpotlightCleaner),
        Box::new(ios_backups::IosBackupsCleaner),
        Box::new(fonts::FontsCleaner),
        Box::new(apps::AppsCleaner),
        Box::new(python_versions::PythonVersionsCleaner),
        Box::new(zsh::ZshCleaner),
        Box::new(projects::ProjectsCleaner),
    ]
}

pub fn cleaner_by_name(name: &str) -> Option<Box<dyn Cleaner>> {
    all_cleaners().into_iter().find(|c| c.name() == name)
}
