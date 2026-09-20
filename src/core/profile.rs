use anyhow::{bail, Result};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fs;
use std::path::PathBuf;

use crate::cleaners::projects::ProjectScanOptions;
use crate::core::RiskLevel;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectProfile {
    pub options: ProjectScanOptions,
    pub risk_ceiling: RiskLevel,
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct ProfileConfig {
    pub profiles: BTreeMap<String, ProjectProfile>,
}

pub fn create_project_profile(
    name: &str,
    options: ProjectScanOptions,
    risk_ceiling: RiskLevel,
) -> Result<PathBuf> {
    validate_name(name)?;
    let mut config = load_config()?;
    config.profiles.insert(
        name.to_string(),
        ProjectProfile {
            options,
            risk_ceiling,
        },
    );
    save_config(&config)
}

pub fn load_project_profile(name: &str) -> Result<ProjectProfile> {
    validate_name(name)?;
    load_config()?
        .profiles
        .remove(name)
        .ok_or_else(|| anyhow::anyhow!("Profile '{}' does not exist.", name))
}

pub fn list() -> Result<Vec<String>> {
    let mut names: Vec<_> = load_config()?.profiles.into_keys().collect();
    names.sort();
    Ok(names)
}

pub fn remove(name: &str) -> Result<()> {
    validate_name(name)?;
    let mut config = load_config()?;
    if config.profiles.remove(name).is_none() {
        bail!("Profile '{}' does not exist.", name);
    }
    save_config(&config)?;
    Ok(())
}

pub fn config_path() -> Result<PathBuf> {
    Ok(crate::core::state_dir()?.join("profiles.toml"))
}

fn load_config() -> Result<ProfileConfig> {
    let path = config_path()?;
    if !path.exists() {
        return Ok(ProfileConfig::default());
    }
    let data = fs::read_to_string(path)?;
    Ok(toml::from_str(&data)?)
}

fn save_config(config: &ProfileConfig) -> Result<PathBuf> {
    let path = config_path()?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(&path, toml::to_string_pretty(config)?)?;
    Ok(path)
}

fn validate_name(name: &str) -> Result<()> {
    if name.is_empty()
        || !name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
    {
        bail!("Profile names may only contain letters, numbers, '-' and '_'.");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_bad_names() {
        assert!(validate_name("../bad").is_err());
        assert!(validate_name("dev-safe").is_ok());
    }
}
