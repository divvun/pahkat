use std::fs::File;
use std::io::Write;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use super::path::ConfigPath;
use super::FileError;
use crate::config::Permission;
use crate::defaults;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SettingsData {
    #[serde(default = "defaults::cache_dir")]
    pub cache_dir: ConfigPath,
    #[serde(default = "defaults::tmp_dir")]
    pub tmp_dir: ConfigPath,
    #[serde(default)]
    pub max_concurrent_downloads: u8,
}

impl Default for SettingsData {
    fn default() -> SettingsData {
        SettingsData {
            cache_dir: defaults::cache_dir(),
            tmp_dir: defaults::tmp_dir(),
            max_concurrent_downloads: 0,
        }
    }
}

impl SettingsData {
    fn load<P: AsRef<Path>>(path: P) -> Result<SettingsData, FileError> {
        let file = std::fs::read_to_string(&path).map_err(|e| FileError::Read(e, path.as_ref().to_path_buf()))?;
        let file = toml::from_str(&file).map_err(|e| FileError::FromToml(e, path.as_ref().to_path_buf()))?;
        Ok(file)
    }

    fn save<P: AsRef<Path>>(&self, path: P) -> Result<(), FileError> {
        let mut file = File::create(&path).map_err(|e| FileError::Write(e, path.as_ref().to_path_buf()))?;
        let b = toml::to_vec(&self).map_err(|e| FileError::ToToml(e, path.as_ref().to_path_buf()))?;
        file.write_all(&b).map_err(|e| FileError::Write(e, path.as_ref().to_path_buf()))?;
        Ok(())
    }

    fn create<P: AsRef<Path>>(path: P) -> Result<SettingsData, FileError> {
        // Create parent directories if they don't exist
        let parent = path.as_ref().parent()
            .ok_or_else(|| FileError::PathParent(path.as_ref().to_path_buf()))?;
        std::fs::create_dir_all(&parent).map_err(|e| FileError::CreateParentDir(e, parent.to_path_buf()))?;

        let file = Self::default();
        file.save(path)?;
        Ok(file)
    }
}

#[derive(Debug, Clone)]
pub struct Settings {
    path: PathBuf,
    data: SettingsData,
    permission: Permission,
}

impl Settings {
    pub fn read_only() -> Settings {
        Settings {
            path: PathBuf::from("/dev/null"),
            data: SettingsData::default(),
            permission: Permission::ReadOnly,
        }
    }

    pub fn new(
        path: PathBuf,
        data: SettingsData,
        permission: Permission,
    ) -> Result<Settings, FileError> {
        let settings = Settings {
            path,
            data,
            permission,
        };

        let package_cache_dir = settings.package_cache_dir();

        if !package_cache_dir.exists() {
            std::fs::create_dir_all(&*package_cache_dir).map_err(|e| FileError::Write(e, settings.path.clone()))?;
        }

        let repo_cache_dir = settings.repo_cache_dir();

        if !repo_cache_dir.exists() {
            std::fs::create_dir_all(&*repo_cache_dir).map_err(|e| FileError::Write(e, settings.path.clone()))?;
        }

        Ok(settings)
    }

    pub fn load<P: AsRef<Path>>(path: P, permission: Permission) -> Result<Settings, FileError> {
        let data = SettingsData::load(path.as_ref())?;
        Self::new(path.as_ref().to_path_buf(), data, permission)
    }

    pub fn create<P: AsRef<Path>>(path: P) -> Result<Settings, FileError> {
        let data = SettingsData::create(path.as_ref())?;
        Self::new(path.as_ref().to_path_buf(), data, Permission::ReadWrite)
    }

    fn reload(&mut self) -> Result<(), FileError> {
        if self.permission == Permission::ReadOnly {
            return Err(FileError::ReadOnly(self.path.clone()));
        }
        self.data = SettingsData::load(&self.path)?;
        Ok(())
    }

    fn save(&self) -> Result<(), FileError> {
        if self.permission == Permission::ReadOnly {
            return Err(FileError::ReadOnly(self.path.clone()));
        }
        self.data.save(&self.path)
    }

    pub fn path(&self) -> &Path {
        self.path.parent().unwrap()
    }

    #[inline(always)]
    fn cache_dir(&self, path: &str) -> ConfigPath {
        self.data.cache_dir.join(path)
    }

    pub(crate) fn config_dir(&self) -> &Path {
        self.path.parent().unwrap()
    }

    pub fn download_cache_dir(&self) -> PathBuf {
        self.cache_dir("downloads").to_path_buf().unwrap()
    }

    pub fn package_cache_dir(&self) -> PathBuf {
        self.cache_dir("packages").to_path_buf().unwrap()
    }

    pub fn repo_cache_dir(&self) -> PathBuf {
        self.cache_dir("repos").to_path_buf().unwrap()
    }

    pub fn cache_base_dir(&self) -> ConfigPath {
        self.data.cache_dir.to_owned()
    }

    pub fn max_concurrent_downloads(&self) -> u8 {
        self.data.max_concurrent_downloads
    }
}
