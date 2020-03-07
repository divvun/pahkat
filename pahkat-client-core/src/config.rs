mod path;
mod repos;
mod settings;

use hashbrown::HashMap;
use std::fmt;
use std::fs::{self, create_dir_all, File};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::{Arc, RwLock};
use url::Url;

use serde::de::{self, Deserializer, Visitor};
use serde::ser::Serializer;
use serde::{Deserialize, Serialize};

use crate::defaults;
use crate::{LoadedRepository, PackageKey};

use once_cell::sync::{Lazy, OnceCell};
use thiserror::Error;

pub use path::ConfigPath;
pub use repos::Repos;
pub use settings::Settings;

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

impl Config {
    #[cfg(not(target_os = "android"))]
    fn load_default() -> Result<Config, Error> {
        let path = defaults::config_path().ok_or(Error::NoDefaultConfigPath)?;
        Self::load(path, false)
    }

    pub fn load<P: AsRef<Path>>(path: P, is_read_only: bool) -> Result<Config, Error> {
        let config_path = path.as_ref();

        let settings_path = config_path.join("settings.toml");

        let settings = match Settings::load(&settings_path, is_read_only) {
            Ok(v) => v,
            Err(FileError::Read(_)) if !is_read_only => {
                Settings::create(&settings_path).map_err(Error::SettingsFile)?
            }
            Err(e) => return Err(Error::SettingsFile(e)),
        };

        let repos_path = config_path.join("repos.toml");

        let repos = match Repos::load(&repos_path, is_read_only) {
            Ok(v) => v,
            Err(FileError::Read(_)) if !is_read_only => {
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

    pub fn settings(&self) -> &Settings {
        &self.settings
    }
}
