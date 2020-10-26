pub(crate) mod path;
mod repos;
mod settings;

pub use path::ConfigPath;
pub use repos::{RepoRecord, Repos, ReposData};
pub use settings::{Settings, SettingsData};

use std::path::{Path, PathBuf};

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

    #[error("An error occurred managing app paths")]
    PathError(#[from] pathos::Error)
}

#[derive(Debug, Error)]
pub enum FileError {
    #[error("The file {0} is read only and could not be written to.")]
    ReadOnly(PathBuf),

    #[error("Could not read file: {1}")]
    Read(#[source] std::io::Error, PathBuf),

    #[error("Could not write file: {1}")]
    Write(#[source] std::io::Error, PathBuf),

    #[error("Could not convert from TOML format: {1}")]
    FromToml(#[source] toml::de::Error, PathBuf),

    #[error("Could not convert into TOML format: {1}")]
    ToToml(#[source] toml::ser::Error, PathBuf),

    #[error("Could not get parent for path: {0}")]
    PathParent(PathBuf),

    #[error("Could not create directory: {1}")]
    CreateParentDir(#[source] std::io::Error, PathBuf),
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
    pub fn read_only() -> Config {
        Config {
            repos: Repos::read_only(),
            settings: Settings::read_only(),
        }
    }

    #[cfg(not(target_os = "android"))]
    pub fn load_default() -> Result<Config, Error> {
        let path = defaults::config_path()?;
        Ok(Self::load(path, Permission::ReadWrite).0)
    }

    pub fn load<P: AsRef<Path>>(path: P, permission: Permission) -> (Config, Vec<Error>) {
        let mut errors = vec![];

        let config_path = path.as_ref();

        let settings_path = config_path.join("settings.toml");

        let settings = match Settings::load(&settings_path, permission) {
            Ok(v) => v,
            Err(_) if permission != Permission::ReadOnly => {
                match Settings::create(&settings_path).map_err(Error::SettingsFile) {
                    Ok(s) => s,
                    Err(e) => {
                        errors.push(e);
                        Settings::read_only()
                    }
                }
            }
            Err(e) => {
                errors.push(Error::SettingsFile(e));
                Settings::read_only()
            },
        };

        let repos_path = config_path.join("repos.toml");

        let repos = match Repos::load(&repos_path, permission) {
            Ok(v) => v,
            Err(_) if permission != Permission::ReadOnly => {
                match Repos::create(&repos_path).map_err(Error::ReposFile) {
                    Ok(s) => s,
                    Err(e) => {
                        errors.push(e);
                        Repos::read_only()
                    }
                }
            }
            Err(e) => {
                errors.push(Error::ReposFile(e));
                Repos::read_only()
            },
        };

        let config = Config { repos, settings };

        log::trace!("Config loaded: {:#?}", &config);

        (config, errors)
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
