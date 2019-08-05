// extern crate reqwest;
// #[cfg(prefix)]
// extern crate rusqlite;
// extern crate serde;
// extern crate serde_json;
// #[macro_use]
// extern crate serde_derive;
// extern crate dirs;
// extern crate semver;
// extern crate tempdir;

// #[cfg(feature = "prefix")]
// extern crate tar;
// #[cfg(feature = "prefix")]
// extern crate xz2;

// #[cfg(all(windows, feature = "windows"))]
// extern crate winreg;

// #[cfg(target_os = "macos")]
// extern crate maplit;
// #[cfg(target_os = "macos")]
// extern crate plist;

use hashbrown::HashMap;
use std::fmt;
use std::fs::{self, create_dir_all, File};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::{Arc, RwLock};
use url::Url;

#[cfg(all(target_os = "macos", feature = "macos"))]
pub mod macos;
#[cfg(feature = "prefix")]
pub mod tarball;
#[cfg(all(windows, feature = "windows"))]
pub mod windows;

mod cmp;
mod download;
pub mod ffi;
pub mod repo;
pub mod transaction;

pub use self::download::Download;
pub use self::repo::Repository;
pub use self::transaction::PackageAction;
use pahkat_types::Repository as RepositoryMeta;

use directories::BaseDirs;

use serde::de::{self, Deserialize, Deserializer, Visitor};
use serde::ser::{Serialize, Serializer};
use serde_derive::{Deserialize, Serialize};

struct AbsolutePackageKeyVisitor;

impl<'de> Visitor<'de> for AbsolutePackageKeyVisitor {
    type Value = AbsolutePackageKey;

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str("an AbsolutePackageKey as a URL string")
    }

    fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        AbsolutePackageKey::from_string(value).map_err(|_| E::custom("Invalid value"))
    }
}

pub fn default_config_path() -> PathBuf {
    BaseDirs::new()
        .expect("base directories must be known")
        .config_dir()
        .join("Pahkat")
}

pub fn default_cache_path() -> PathBuf {
    BaseDirs::new()
        .expect("base directories must be known")
        .cache_dir()
        .join("Pahkat")
}

pub fn default_tmp_path() -> PathBuf {
    default_cache_path().join("tmp")
}

#[cfg(target_os = "macos")]
pub fn default_uninstall_path() -> PathBuf {
    BaseDirs::new()
        .expect("base directories must be known")
        .data_dir()
        .join("Pahkat")
        .join("uninstall")
}

#[cfg(target_os = "macos")]
pub fn global_uninstall_path() -> PathBuf {
    PathBuf::from("/Library/Application Support/Pahkat/uninstall")
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq, Hash)]
pub struct RepoRecord {
    #[serde(with = "url_serde")]
    pub url: Url,
    pub channel: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct AbsolutePackageKey {
    pub url: Url,
    pub id: String,
    pub channel: String,
}

impl Serialize for AbsolutePackageKey {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&*self.to_string())
    }
}

impl<'de> Deserialize<'de> for AbsolutePackageKey {
    fn deserialize<D>(deserializer: D) -> Result<AbsolutePackageKey, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_str(AbsolutePackageKeyVisitor)
    }
}

impl AbsolutePackageKey {
    pub fn new(repo: &RepositoryMeta, channel: &str, package_id: &str) -> AbsolutePackageKey {
        AbsolutePackageKey {
            url: Url::parse(&repo.base).expect("repo base url must be valid"),
            id: package_id.to_string(),
            channel: channel.to_string(),
        }
    }

    // TODO impl From trait.
    pub fn to_string(&self) -> String {
        format!("{}packages/{}#{}", self.url, self.id, self.channel)
    }

    pub fn from_string(url: &str) -> Result<AbsolutePackageKey, Box<dyn std::error::Error>> {
        let url = Url::parse(url)?;

        let channel = url.fragment().unwrap().to_string();
        let base = url.join("..")?;
        let id = url.path_segments().unwrap().last().unwrap().to_string();

        Ok(AbsolutePackageKey {
            url: base,
            channel,
            id,
        })
    }
}

// use url::Url;

pub enum ConfigPath {
    Container(Url),
    File(Url),
}

impl ConfigPath {
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
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
struct RawStoreConfig {
    #[serde(default = "Vec::new")]
    pub repos: Vec<RepoRecord>,
    #[serde(default = "HashMap::new")]
    pub skipped_packages: HashMap<AbsolutePackageKey, String>,
    #[serde(default = "default_cache_path")]
    pub cache_path: PathBuf,
    #[serde(default = "default_tmp_path")]
    pub tmp_path: PathBuf,
    #[serde(default = "HashMap::new")]
    pub ui: HashMap<String, String>,
}

impl std::default::Default for RawStoreConfig {
    fn default() -> RawStoreConfig {
        RawStoreConfig {
            repos: vec![],
            skipped_packages: HashMap::new(),
            cache_path: default_cache_path(),
            tmp_path: default_tmp_path(),
            ui: HashMap::new(),
        }
    }
}

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
            config_path: default_config_path().join("config.json"),
            data: Arc::new(RwLock::new(RawStoreConfig::default())),
            save_changes: true,
        }
    }
}

type SaveResult = Result<(), Box<dyn std::error::Error>>;

impl StoreConfig {
    pub fn load_or_default(save_changes: bool) -> StoreConfig {
        let res = StoreConfig::load(&default_config_path().join("config.json"), save_changes);

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

    pub fn skipped_package(&self, key: &AbsolutePackageKey) -> Option<String> {
        self.data
            .read()
            .unwrap()
            .skipped_packages
            .get(key)
            .map(|x| x.to_string())
    }

    pub fn remove_skipped_package(&self, key: &AbsolutePackageKey) -> SaveResult {
        {
            self.data.write().unwrap().skipped_packages.remove(key);
        }

        if self.save_changes {
            self.save()
        } else {
            Ok(())
        }
    }

    pub fn add_skipped_package(&self, key: AbsolutePackageKey, version: String) -> SaveResult {
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

    pub fn package_cache_path(&self) -> PathBuf {
        self.data.read().unwrap().cache_path.join("packages")
    }

    pub fn repo_cache_path(&self) -> PathBuf {
        self.data.read().unwrap().cache_path.join("repos")
    }

    // pub(crate) fn cache_base_path(&self) -> PathBuf {
    //     self.data.read().unwrap().cache_path.to_owned()
    // }

    pub fn set_cache_base_path(&self, cache_path: PathBuf) -> SaveResult {
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
