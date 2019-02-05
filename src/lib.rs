extern crate pahkat;
#[cfg(prefix)]
extern crate rusqlite;
extern crate reqwest;
extern crate serde_json;
extern crate serde;
#[macro_use]
extern crate serde_derive;
extern crate semver;
extern crate tempdir;
extern crate dirs;

#[cfg(feature = "prefix")]
extern crate rhai;
#[cfg(feature = "prefix")]
extern crate xz2;
#[cfg(feature = "prefix")]
extern crate tar;

#[cfg(windows)]
extern crate winreg;

#[cfg(target_os = "macos")]
extern crate plist;
#[cfg(target_os = "macos")]
extern crate maplit;

#[cfg(windows)]
extern crate winapi;

extern crate crypto;
extern crate sentry;

use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::fs::{self, create_dir_all, File};
use std::fmt;
use std::collections::HashMap;
use url::Url;
use std::sync::{Arc, RwLock};

#[cfg(windows)]
pub mod windows;
#[cfg(target_os = "macos")]
pub mod macos;
pub mod tarball;

pub mod ffi;
mod cmp;
mod download;
pub mod repo;
pub use self::download::Download;
pub use self::repo::Repository;
use pahkat::types::{
    Repository as RepositoryMeta
};

use directories::BaseDirs;

#[derive(Debug)]
pub enum TransactionEvent {
    NotStarted,
    Uninstalling,
    Installing,
    Completed,
    Error
}

impl TransactionEvent {
    pub fn to_u32(&self) -> u32 {
        match self {
            TransactionEvent::NotStarted => 0,
            TransactionEvent::Uninstalling => 1,
            TransactionEvent::Installing => 2,
            TransactionEvent::Completed => 3,
            TransactionEvent::Error => 4
        }
    }
}

#[derive(Debug, Clone)]
pub enum PackageTransactionError {
    NoPackage(String),
    Deps(PackageDependencyError),
    ActionContradiction(String)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum PackageActionType {
    Install,
    Uninstall
}

impl PackageActionType {
    pub fn from_u8(x: u8) -> PackageActionType {
        match x {
            0 => PackageActionType::Install,
            1 => PackageActionType::Uninstall,
            _ => panic!("Invalid package action type: {}", x)
        }
    }

    pub fn to_u8(&self) -> u8 {
        match self {
            PackageActionType::Install => 0,
            PackageActionType::Uninstall => 1
        }
    }
}

use serde::ser::{Serialize, Serializer};
use serde::de::{self, Visitor, Deserialize, Deserializer};

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


#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum PackageStatus {
    NotInstalled,
    UpToDate,
    RequiresUpdate,
    Skipped
}

impl PackageStatus {
    fn to_u8(&self) -> u8 {
        match self {
            PackageStatus::NotInstalled => 0,
            PackageStatus::UpToDate => 1,
            PackageStatus::RequiresUpdate => 2,
            PackageStatus::Skipped => 3
        }
    }
}

impl fmt::Display for PackageStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "{}", match *self {
            // PackageStatus::NoPackage => "No package",
            PackageStatus::NotInstalled => "Not installed",
            PackageStatus::UpToDate => "Up to date",
            PackageStatus::RequiresUpdate => "Requires update",
            PackageStatus::Skipped => "Skipped"
        })
    }
}

#[derive(Debug, Clone, Copy)]
pub enum PackageStatusError {
    NoPackage,
    NoInstaller,
    WrongInstallerType,
    ParsingVersion,
    InvalidInstallPath,
    InvalidMetadata
}

impl fmt::Display for PackageStatusError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "Error: {}", match *self {
            PackageStatusError::NoPackage => "No package",
            PackageStatusError::NoInstaller => "No installer",
            PackageStatusError::WrongInstallerType => "Wrong installer type",
            PackageStatusError::ParsingVersion => "Could not parse version",
            PackageStatusError::InvalidInstallPath => "Invalid install path",
            PackageStatusError::InvalidMetadata => "Invalid metadata"
        })
    }
}

pub fn default_config_path() -> PathBuf {
    BaseDirs::new().expect("base directories must be known")
        .config_dir().join("Pahkat")
}

pub fn default_cache_path() -> PathBuf {
    BaseDirs::new().expect("base directories must be known")
        .cache_dir().join("Pahkat")
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq, Hash)]
pub struct RepoRecord {
    #[serde(with = "url_serde")]
    pub url: Url,
    pub channel: String
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct AbsolutePackageKey {
    pub url: Url,
    pub id: String,
    pub channel: String
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
            channel: channel.to_string()
        }
    }
    
    // TODO impl From trait.
    pub fn to_string(&self) -> String {
        format!("{}packages/{}#{}", self.url, self.id, self.channel)
    }

    pub fn from_string(url: &str) -> Result<AbsolutePackageKey, ()> {
        let url = Url::parse(url).unwrap();

        let channel = url.fragment().unwrap().to_string();
        let base = url.join("..").unwrap();
        let id = url.path_segments().unwrap().last().unwrap().to_string();

        Ok(AbsolutePackageKey {
            url: base,
            channel,
            id
        })
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
    #[serde(default = "HashMap::new")]
    pub ui: HashMap<String, String>
}

impl std::default::Default for RawStoreConfig {
    fn default() -> RawStoreConfig {
        RawStoreConfig {
            repos: vec![],
            skipped_packages: HashMap::new(),
            cache_path: default_cache_path(),
            ui: HashMap::new()
        }
    }
}

#[derive(Debug, Clone)]
pub struct StoreConfig {
    /// A reference to the path for this StoreConfig
    config_path: PathBuf,
    data: Arc<RwLock<RawStoreConfig>>,
    save_changes: bool
}

impl std::default::Default for StoreConfig {
    fn default() -> StoreConfig {
        StoreConfig {
            config_path: default_config_path().join("config.json"),
            data: Arc::new(RwLock::new(RawStoreConfig::default())),
            save_changes: true
        }
    }
}

// TODO no unwrap
impl StoreConfig {
    pub fn load_or_default(save_changes: bool) -> StoreConfig {
        let res = StoreConfig::load(&default_config_path().join("config.json"), save_changes);
        
        let mut config = match res {
            Ok(v) => v,
            Err(_) => StoreConfig::default()
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

    pub fn load(config_path: &Path, save_changes: bool) -> io::Result<StoreConfig> {
        let file = File::open(config_path)?;
        let data: RawStoreConfig = serde_json::from_reader(file)?;

        Ok(StoreConfig {
            config_path: config_path.to_owned(),
            data: Arc::new(RwLock::new(data)),
            save_changes
        })
    }

    pub fn save(&self) -> Result<(), ()> { 
        let cfg_str = serde_json::to_string_pretty(&*self.data.read().unwrap()).unwrap();
        {
            create_dir_all(self.config_path.parent().unwrap()).unwrap();
            let mut file = File::create(&self.config_path).unwrap();
            file.write_all(cfg_str.as_bytes()).unwrap();
        }

        Ok(())
    }
    
    pub fn config_path(&self) -> &Path {
        &self.config_path
    }

    pub fn skipped_package(&self, key: &AbsolutePackageKey) -> Option<String> {
        self.data.read().unwrap().skipped_packages.get(key).map(|x| x.to_string())
    }

    pub fn remove_skipped_package(&self, key: &AbsolutePackageKey) -> Result<(), ()> {
        {
            self.data.write().unwrap().skipped_packages.remove(key);
        }

        if self.save_changes {
            self.save()
        } else { 
            Ok(())
        }
    }

    pub fn add_skipped_package(&self, key: AbsolutePackageKey, version: String) -> Result<(), ()> {
        {
            self.data.write().unwrap().skipped_packages.insert(key, version);
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

    pub(crate) fn cache_base_path(&self) -> PathBuf {
        self.data.read().unwrap().cache_path.to_owned()
    }

    pub fn set_cache_base_path(&self, cache_path: PathBuf) -> Result<(), ()> {
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

    pub fn set_repos(&self, repos: Vec<RepoRecord>) -> Result<(), ()> {
        {
            self.data.write().unwrap().repos = repos
        }

        if self.save_changes {
            self.save()
        } else { 
            Ok(())
        }
    }

    pub fn add_repo(&self, repo_record: RepoRecord) -> Result<(), ()> {
        {
            self.data.write().unwrap().repos.push(repo_record);
        }
        if self.save_changes {
            self.save()
        } else { 
            Ok(())
        }
    }

    pub fn remove_repo(&self, repo_record: RepoRecord) -> Result<(), ()> {
        match self.data.read().unwrap().repos.iter().position(|r| r == &repo_record) {
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
            },
            None => Ok(())
        }
    }

    pub fn update_repo(&self, index: usize, repo_record: RepoRecord) -> Result<(), ()> {
        {
            self.data.write().unwrap().repos[index] = repo_record;
        }
        if self.save_changes {
            self.save()
        } else { 
            Ok(())
        }
    }

    pub fn set_ui_setting(&self, key: &str, value: Option<String>) -> Result<(), ()> {
        println!("Set UI setting: {} -> {:?}", key, &value);
        {
            let mut lock = self.data.write().expect("write lock");
            match value {
                Some(v) => lock.ui.insert(key.to_string(), v),
                None => lock.ui.remove(key)
            };
        }
        if self.save_changes {
            self.save()
        } else { 
            Ok(())
        }
    }

    pub fn ui_setting(&self, key: &str) -> Option<String> {
        self.data.read().unwrap().ui.get(key).map(|x| x.to_string())
    }
}

#[derive(Debug, Clone, Copy)]
pub enum PackageDependencyError {
    PackageNotFound,
    VersionNotFound,
    PackageStatusError(PackageStatusError)
}

impl fmt::Display for PackageDependencyError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
       match *self {
            PackageDependencyError::PackageNotFound => write!(f, "Error: Package not found"),
            PackageDependencyError::VersionNotFound => write!(f, "Error: Package version not found"),
            PackageDependencyError::PackageStatusError(e) => write!(f, "{}", e),
        }
    }
}

#[derive(Debug)]
pub struct PackageDependency {
    pub id: AbsolutePackageKey,
    pub version: String,
    pub level: u8,
    pub status: PackageStatus
}
