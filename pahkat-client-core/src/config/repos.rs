use std::fs::File;
use std::io::Write;
use std::path::{Path, PathBuf};

use indexmap::IndexMap;
use serde::{Deserialize, Serialize};

use super::FileError;
use crate::config::Permission;
use pahkat_types::repo::RepoUrl;

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct RepoRecord {
    pub channel: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(transparent)]
pub struct ReposData(IndexMap<RepoUrl, RepoRecord>);

impl ReposData {
    fn load<P: AsRef<Path>>(path: P) -> Result<ReposData, FileError> {
        let file = std::fs::read_to_string(&path)
            .map_err(|e| FileError::Read(e, path.as_ref().to_path_buf()))?;
        let file = toml::from_str(&file)
            .map_err(|e| FileError::FromToml(e, path.as_ref().to_path_buf()))?;
        Ok(file)
    }

    fn save<P: AsRef<Path>>(&self, path: P) -> Result<(), FileError> {
        let mut file =
            File::create(&path).map_err(|e| FileError::Write(e, path.as_ref().to_path_buf()))?;
        let b =
            toml::to_vec(&self).map_err(|e| FileError::ToToml(e, path.as_ref().to_path_buf()))?;
        file.write_all(&b)
            .map_err(|e| FileError::Write(e, path.as_ref().to_path_buf()))?;
        Ok(())
    }

    fn create<P: AsRef<Path>>(path: P) -> Result<ReposData, FileError> {
        // Create parent directories if they don't exist
        let parent = path
            .as_ref()
            .parent()
            .ok_or_else(|| FileError::PathParent(path.as_ref().to_path_buf()))?;
        std::fs::create_dir_all(&parent)
            .map_err(|e| FileError::CreateParentDir(e, parent.to_path_buf()))?;

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
    type Target = IndexMap<RepoUrl, RepoRecord>;

    fn deref(&self) -> &Self::Target {
        &self.data.0
    }
}

impl std::ops::DerefMut for Repos {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.data.0
    }
}

impl Repos {
    pub fn read_only() -> Repos {
        Repos {
            path: PathBuf::from("/dev/null"),
            data: ReposData::default(),
            permission: Permission::ReadOnly,
        }
    }

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
            return Err(FileError::ReadOnly(self.path.clone()));
        }
        self.data = ReposData::load(&self.path)?;
        Ok(())
    }

    fn save(&self) -> Result<(), FileError> {
        if self.permission == Permission::ReadOnly {
            return Err(FileError::ReadOnly(self.path.clone()));
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

    pub fn insert(&mut self, key: RepoUrl, value: RepoRecord) -> Result<(), FileError> {
        self.data.0.insert(key, value);

        if self.permission == Permission::ReadWrite {
            return self.data.save(&self.path);
        }

        Ok(())
    }

    pub fn remove(&mut self, key: &RepoUrl) -> Result<bool, FileError> {
        let result = self.data.0.remove(key).is_some();

        if self.permission == Permission::ReadWrite {
            self.data.save(&self.path)?;
        }

        Ok(result)
    }

    pub fn data(&self) -> &ReposData {
        &self.data
    }
}
