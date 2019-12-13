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
use crate::{repo::RepoRecord, PackageKey, Repository};

#[derive(Debug, Clone)]
pub struct StoreConfig {
    /// A reference to the path for this StoreConfig
    config_path: PathBuf,
    data: Arc<RwLock<RawStoreConfig>>,
    save_changes: bool,
}

impl std::default::Default for StoreConfig {
    fn default() -> StoreConfig {
        StoreConfig {
            config_path: defaults::config_path().join("config.json"),
            data: Arc::new(RwLock::new(RawStoreConfig::default())),
            save_changes: true,
        }
    }
}

type SaveResult = Result<(), Box<dyn std::error::Error>>;

impl StoreConfig {
    pub fn load_or_default(save_changes: bool) -> StoreConfig {
        let res = StoreConfig::load(&defaults::config_path().join("config.json"), save_changes);

        let mut config = match res {
            Ok(v) => v,
            Err(_) => StoreConfig::default(),
        };
        config.save_changes = save_changes;

        if !config.package_cache_path().exists() {
            std::fs::create_dir_all(&*config.package_cache_path()).unwrap();
        }

        if !config.repo_cache_path().exists() {
            std::fs::create_dir_all(&*config.repo_cache_path()).unwrap();
        }

        config
    }

    pub fn new(config_path: &Path) -> StoreConfig {
        StoreConfig {
            config_path: config_path.join("config.json"),
            data: Arc::new(RwLock::new(RawStoreConfig::default())),
            save_changes: true,
        }
    }

    pub fn load(config_path: &Path, save_changes: bool) -> io::Result<StoreConfig> {
        log::debug!("StoreConfig::load({:?}, {})", config_path, save_changes);

        let file = File::open(config_path)?;
        let data: RawStoreConfig = serde_json::from_reader(file)?;

        log::debug!("Data: {:?}", data);

        Ok(StoreConfig {
            config_path: config_path.to_owned(),
            data: Arc::new(RwLock::new(data)),
            save_changes,
        })
    }

    pub fn save(&self) -> Result<(), Box<dyn std::error::Error>> {
        let cfg_str = serde_json::to_string_pretty(&*self.data.read().unwrap())?;
        {
            log::debug!("Saving: {:?}", self.config_path);
            create_dir_all(self.config_dir()).unwrap();
            let mut file = File::create(&self.config_path).unwrap();
            file.write_all(cfg_str.as_bytes())?;
        }

        Ok(())
    }

    pub fn config_path(&self) -> &Path {
        &self.config_path
    }

    pub fn tmp_path(&self) -> PathBuf {
        self.data.read().unwrap().tmp_path.to_path_buf()
    }

    pub fn config_dir(&self) -> &Path {
        &self.config_path.parent().expect("parent dir must exist")
    }

    pub fn skipped_package(&self, key: &PackageKey) -> Option<String> {
        self.data
            .read()
            .unwrap()
            .skipped_packages
            .get(key)
            .map(|x| x.to_string())
    }

    pub fn remove_skipped_package(&self, key: &PackageKey) -> SaveResult {
        {
            self.data.write().unwrap().skipped_packages.remove(key);
        }

        if self.save_changes {
            self.save()
        } else {
            Ok(())
        }
    }

    pub fn add_skipped_package(&self, key: PackageKey, version: String) -> SaveResult {
        {
            self.data
                .write()
                .unwrap()
                .skipped_packages
                .insert(key, version);
        }
        if self.save_changes {
            self.save()
        } else {
            Ok(())
        }
    }

    pub fn download_cache_path(&self) -> PathBuf {
        self.data.read().unwrap().cache_path.join("downloads")
    }

    pub fn package_cache_path(&self) -> PathBuf {
        self.data.read().unwrap().cache_path.join("packages")
    }

    pub fn repo_cache_path(&self) -> PathBuf {
        self.data.read().unwrap().cache_path.join("repos")
    }

    pub fn cache_base_path(&self) -> ConfigPath {
        self.data.read().unwrap().cache_path.to_owned()
    }

    pub fn set_max_concurrent_downloads(&self, value: u8) -> SaveResult {
        {
            self.data.write().unwrap().max_concurrent_downloads = value;
        }
        if self.save_changes {
            self.save()
        } else {
            Ok(())
        }
    }

    pub fn max_concurrent_downloads(&self) -> u8 {
        self.data.read().unwrap().max_concurrent_downloads
    }

    pub fn set_cache_base_path(&self, cache_path: ConfigPath) -> SaveResult {
        {
            self.data.write().unwrap().cache_path = cache_path;
        }
        if self.save_changes {
            self.save()
        } else {
            Ok(())
        }
    }

    pub fn repos(&self) -> Vec<RepoRecord> {
        self.data.read().unwrap().repos.clone()
    }

    pub fn set_repos(&self, repos: Vec<RepoRecord>) -> SaveResult {
        {
            self.data.write().unwrap().repos = repos
        }

        if self.save_changes {
            self.save()
        } else {
            Ok(())
        }
    }

    pub fn add_repo(&self, repo_record: RepoRecord) -> SaveResult {
        {
            self.data.write().unwrap().repos.push(repo_record);
        }
        if self.save_changes {
            self.save()
        } else {
            Ok(())
        }
    }

    pub fn remove_repo(&self, repo_record: RepoRecord) -> SaveResult {
        match self
            .data
            .read()
            .unwrap()
            .repos
            .iter()
            .position(|r| r == &repo_record)
        {
            Some(index) => {
                self.data.write().unwrap().repos.remove(index);

                let hash_id = Repository::path_hash(&repo_record.url, &repo_record.channel);
                let cache_path = self.repo_cache_path().join(hash_id);
                if cache_path.exists() {
                    fs::remove_dir_all(cache_path).expect("cache dir deleted");
                }
                if self.save_changes {
                    self.save()
                } else {
                    Ok(())
                }
            }
            None => Ok(()),
        }
    }

    pub fn update_repo(&self, index: usize, repo_record: RepoRecord) -> SaveResult {
        {
            self.data.write().unwrap().repos[index] = repo_record;
        }
        if self.save_changes {
            self.save()
        } else {
            Ok(())
        }
    }

    pub fn set_ui_value(&self, key: &str, value: Option<String>) -> SaveResult {
        log::debug!("Set UI setting: {} -> {:?}", key, &value);
        {
            let mut lock = self.data.write().expect("write lock");
            match value {
                Some(v) => lock.ui.insert(key.to_string(), v),
                None => lock.ui.remove(key),
            };
        }
        if self.save_changes {
            self.save()
        } else {
            Ok(())
        }
    }

    pub fn ui_value(&self, key: &str) -> Option<String> {
        self.data.read().unwrap().ui.get(key).map(|x| x.to_string())
    }
}

#[derive(Debug, Clone, PartialEq, Hash)]
pub enum ConfigPath {
    Container(Url),
    File(Url),
}

#[derive(Debug, Clone)]
pub enum ConfigPathError {
    InvalidScheme(String),
    InvalidUrl,
}

impl std::error::Error for ConfigPathError {}

impl std::fmt::Display for ConfigPathError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "{:?}", self)
    }
}

impl ConfigPath {
    pub fn from_path<P: AsRef<Path>>(path: P) -> Result<ConfigPath, Box<dyn std::error::Error>> {
        Url::from_file_path(path)
            .map(|url| ConfigPath::File(url))
            .map_err(|_| Box::new(ConfigPathError::InvalidUrl) as _)
    }

    pub fn from_url(url: Url) -> Result<ConfigPath, Box<dyn std::error::Error>> {
        let scheme = url.scheme();
        if scheme == "file" {
            Ok(ConfigPath::File(url))
        } else if scheme == "container" {
            Ok(ConfigPath::Container(url))
        } else {
            Err(Box::new(ConfigPathError::InvalidScheme(scheme.to_string())))
        }
    }

    pub fn join<P: AsRef<Path>>(&self, path: P) -> PathBuf {
        self.to_path_buf().join(path.as_ref())
    }

    pub fn to_path_buf(&self) -> PathBuf {
        self.try_as_path().unwrap()
    }

    fn container_to_file(&self) -> Option<Url> {
        let url = match self {
            ConfigPath::File(v) => return Some(v.to_owned()),
            ConfigPath::Container(v) => v,
        };

        let container_path = dirs::home_dir().expect("valid home dir").join(url.path());
        Url::from_file_path(container_path).ok()
    }

    fn try_as_path(&self) -> Option<PathBuf> {
        let url = match self {
            ConfigPath::File(ref v) => v.to_owned(),
            ConfigPath::Container(_v) => self.container_to_file()?,
        };

        url.to_file_path().ok()
    }

    pub fn as_url(&self) -> &Url {
        match self {
            ConfigPath::File(url) => url,
            ConfigPath::Container(url) => url,
        }
    }
}

impl Serialize for ConfigPath {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&*self.as_url().to_string())
    }
}

impl<'de> Deserialize<'de> for ConfigPath {
    fn deserialize<D>(deserializer: D) -> Result<ConfigPath, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_str(ConfigPathVisitor)
    }
}

struct ConfigPathVisitor;

impl<'de> Visitor<'de> for ConfigPathVisitor {
    type Value = ConfigPath;

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str("a ConfigPath as a URL string")
    }

    fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        if value.starts_with("file:") || value.starts_with("container:") {
            let url = url::Url::parse(value).map_err(|_| E::custom("Invalid URL"))?;
            ConfigPath::from_url(url).map_err(|_| E::custom("Invalid URL scheme"))
        } else {
            ConfigPath::from_path(value).map_err(|_| E::custom("Invalid file path"))
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
struct RawStoreConfig {
    #[serde(default = "Vec::new")]
    pub repos: Vec<RepoRecord>,
    #[serde(default = "HashMap::new")]
    pub skipped_packages: HashMap<PackageKey, String>,
    #[serde(default = "defaults::cache_path")]
    pub cache_path: ConfigPath,
    #[serde(default = "defaults::tmp_path")]
    pub tmp_path: ConfigPath,
    #[serde(default)]
    pub max_concurrent_downloads: u8,
    #[serde(default = "HashMap::new")]
    pub ui: HashMap<String, String>,
}

impl std::default::Default for RawStoreConfig {
    fn default() -> RawStoreConfig {
        RawStoreConfig {
            repos: vec![],
            skipped_packages: HashMap::new(),
            cache_path: defaults::cache_path(),
            tmp_path: defaults::tmp_path(),
            max_concurrent_downloads: 3,
            ui: HashMap::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse() {
        let container = ConfigPath::Container(url::Url::parse("container:test").unwrap());
        let fp = ConfigPath::File(url::Url::parse("file:///foo").unwrap());
        assert_eq!(
            &std::env::home_dir().unwrap().join("test"),
            &*container.to_path_buf()
        );
        assert_eq!(std::path::Path::new("/foo"), &*fp.to_path_buf());
    }
}
