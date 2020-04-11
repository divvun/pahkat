use std::fs::File;
use std::io::Write;
use std::path::{Path, PathBuf};

use indexmap::IndexMap;
use serde::{Deserialize, Serialize};
use url::Url;

use crate::config::Permission;
use super::FileError;

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct RepoRecord {
    pub channel: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(transparent)]
pub struct ReposData(IndexMap<Url, RepoRecord>);

impl ReposData {
    fn load<P: AsRef<Path>>(path: P) -> Result<ReposData, FileError> {
        let file = std::fs::read_to_string(path).map_err(FileError::Read)?;
        let file = toml::from_str(&file)?;
        Ok(file)
    }

    fn save<P: AsRef<Path>>(&self, path: P) -> Result<(), FileError> {
        let mut file = File::create(path).map_err(FileError::Write)?;
        let b = toml::to_vec(&self)?;
        file.write_all(&b).map_err(FileError::Write)?;
        Ok(())
    }

    fn create<P: AsRef<Path>>(path: P) -> Result<ReposData, FileError> {
        let file = Self::default();
        file.save(path)?;
        Ok(file)
    }

    #[inline]
    pub fn len(&self) -> usize {
        self.0.len()
    }
}

#[derive(Debug, Clone)]
pub struct Repos {
    path: PathBuf,
    data: ReposData,
    permission: Permission,
}

impl std::ops::Deref for Repos {
    type Target = IndexMap<Url, RepoRecord>;

    fn deref(&self) -> &Self::Target {
        &self.data.0
    }
}

impl Repos {
    pub fn create<P: AsRef<Path>>(path: P) -> Result<Repos, FileError> {
        let data = ReposData::create(path.as_ref())?;

        Ok(Repos {
            path: path.as_ref().to_path_buf(),
            data,
            permission: Permission::ReadWrite,
        })
    }

    pub fn load<P: AsRef<Path>>(path: P, permission: Permission) -> Result<Repos, FileError> {
        let data = ReposData::load(path.as_ref())?;

        Ok(Repos {
            path: path.as_ref().to_path_buf(),
            data,
            permission,
        })
    }

    fn reload(&mut self) -> Result<(), FileError> {
        if self.permission == Permission::ReadOnly {
            return Err(FileError::ReadOnly);
        }
        self.data = ReposData::load(&self.path)?;
        Ok(())
    }

    fn save(&self) -> Result<(), FileError> {
        if self.permission == Permission::ReadOnly {
            return Err(FileError::ReadOnly);
        }
        self.data.save(&self.path)
    }

    pub fn set(&mut self, data: ReposData) -> Result<(), FileError> {
        self.data = data;

        if self.permission == Permission::ReadWrite {
            return self.data.save(&self.path);
        }

        Ok(())
    }

    pub fn insert(&mut self, key: Url, value: RepoRecord) -> Result<(), FileError> {
        self.data.0.insert(key, value);

        if self.permission == Permission::ReadWrite {
            return self.data.save(&self.path);
        }

        Ok(())
    }

    pub fn get(&self) -> &ReposData {
        &self.data
    }
}
