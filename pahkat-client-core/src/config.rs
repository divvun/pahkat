mod path;
mod repos;
mod settings;

pub use path::ConfigPath;
pub use repos::{RepoRecord, Repos, ReposData};
pub use settings::{Settings, SettingsData};

use std::path::Path;

use thiserror::Error;

use crate::defaults;

#[derive(Debug, Error)]
pub enum Error {
    #[error("No default configuration path found for this platform")]
    NoDefaultConfigPath,

    #[error("Error loading repos.toml file")]
    ReposFile(#[source] FileError),

    #[error("Error loading settings.toml file")]
    SettingsFile(#[source] FileError),
}

#[derive(Debug, Error)]
pub enum FileError {
    #[error("The file is read only and could not be written to")]
    ReadOnly,

    #[error("Could not read file")]
    Read(#[source] std::io::Error),

    #[error("Could not write file")]
    Write(#[source] std::io::Error),

    #[error("Could not convert from TOML format")]
    FromToml(#[from] toml::de::Error),

    #[error("Could not convert into TOML format")]
    ToToml(#[from] toml::ser::Error),
}

#[derive(Debug, Clone)]
pub struct Config {
    repos: Repos,
    settings: Settings,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Permission {
    ReadOnly,
    ReadWrite,
}

impl Config {
    #[cfg(not(target_os = "android"))]
    pub fn load_default() -> Result<Config, Error> {
        let path = defaults::config_path().ok_or(Error::NoDefaultConfigPath)?;
        Self::load(path, Permission::ReadWrite)
    }

    pub fn load<P: AsRef<Path>>(path: P, permission: Permission) -> Result<Config, Error> {
        let config_path = path.as_ref();

        let settings_path = config_path.join("settings.toml");

        let settings = match Settings::load(&settings_path, permission) {
            Ok(v) => v,
            Err(FileError::Read(_)) if permission != Permission::ReadOnly => {
                Settings::create(&settings_path).map_err(Error::SettingsFile)?
            }
            Err(e) => return Err(Error::SettingsFile(e)),
        };

        let repos_path = config_path.join("repos.toml");

        let repos = match Repos::load(&repos_path, permission) {
            Ok(v) => v,
            Err(FileError::Read(_)) if permission != Permission::ReadOnly => {
                Repos::create(&repos_path).map_err(Error::ReposFile)?
            }
            Err(e) => return Err(Error::ReposFile(e)),
        };
        
        let config = Config { repos, settings };

        Ok(config)
    }

    pub fn new(settings: Settings, repos: Repos) -> Config {
        Config { repos, settings }
    }

    pub fn repos(&self) -> &Repos {
        &self.repos
    }

    pub fn repos_mut(&mut self) -> &mut Repos {
        &mut self.repos
    }

    pub fn settings(&self) -> &Settings {
        &self.settings
    }

    pub fn settings_mut(&mut self) -> &mut Settings {
        &mut self.settings
    }
}
