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

#[cfg(feature = "ipc")]
extern crate jsonrpc_core;
#[cfg(feature = "ipc")]
extern crate jsonrpc_pubsub;
#[macro_use]
#[cfg(feature = "ipc")]
extern crate jsonrpc_macros;

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
use std::fs::{create_dir_all, File};
use std::fmt;
use std::cell::{Ref, RefCell};
use std::collections::HashMap;
use url::Url;
use std::sync::{Arc, RwLock};

#[cfg(windows)]
pub mod windows;
#[cfg(target_os = "macos")]
pub mod macos;
pub mod tarball;

pub mod ffi;
mod download;
pub mod repo;
pub use self::download::Download;
pub use self::repo::Repository;

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
            PackageStatus::NotInstalled => "Not installed",
            PackageStatus::UpToDate => "Up to date",
            PackageStatus::RequiresUpdate => "Requires update",
            PackageStatus::Skipped => "Skipped"
        })
    }
}

#[derive(Debug, Clone, Copy)]
pub enum PackageStatusError {
    NoInstaller,
    WrongInstallerType,
    ParsingVersion,
    InvalidInstallPath,
    InvalidMetadata
}

impl fmt::Display for PackageStatusError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "Error: {}", match *self {
            PackageStatusError::NoInstaller => "No installer",
            PackageStatusError::WrongInstallerType => "Wrong installer type",
            PackageStatusError::ParsingVersion => "Could not parse version",
            PackageStatusError::InvalidInstallPath => "Invalid install path",
            PackageStatusError::InvalidMetadata => "Invalid metadata"
        })
    }
}

// TODO Use directories crate
#[cfg(target_os = "macos")]
pub fn default_config_path() -> PathBuf {
    dirs::home_dir().unwrap().join("Library/Application Support/Pahkat/config.json")
}

#[cfg(target_os = "linux")]
pub fn default_config_path() -> PathBuf {
    dirs::home_dir().unwrap().join(".config/pahkat/config.json")
}

#[cfg(windows)]
pub fn default_config_path() -> PathBuf {
    dirs::home_dir().unwrap().join(r#"AppData\Roaming\Pahkat\config.json"#)
}

#[cfg(target_os = "macos")]
pub fn default_cache_path() -> PathBuf {
    dirs::home_dir().unwrap().join("Library/Caches/Pahkat/packages")
}

#[cfg(target_os = "linux")]
pub fn default_cache_path() -> PathBuf {
    dirs::home_dir().unwrap().join(".cache/pahkat/packages")
}

#[cfg(windows)]
pub fn default_cache_path() -> PathBuf {
    dirs::home_dir().unwrap().join(r#"AppData\Local\Pahkat\packages"#)
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq, Hash)]
pub struct RepoRecord {
    #[serde(with = "url_serde")]
    pub url: Url,
    pub channel: String
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq, Hash)]
pub struct AbsolutePackageKey {
    #[serde(with = "url_serde")]
    pub url: Url,
    pub id: String,
    pub channel: String
}

impl AbsolutePackageKey {
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
    pub cache_path: PathBuf
}

impl std::default::Default for RawStoreConfig {
    fn default() -> RawStoreConfig {
        RawStoreConfig {
            repos: vec![],
            skipped_packages: HashMap::new(),
            cache_path: default_cache_path()
        }
    }
}

#[derive(Debug, Clone)]
pub struct StoreConfig {
    /// A reference to the path for this StoreConfig
    config_path: PathBuf,
    data: Arc<RwLock<RawStoreConfig>>
}

impl std::default::Default for StoreConfig {
    fn default() -> StoreConfig {
        StoreConfig {
            config_path: default_config_path(),
            data: Arc::new(RwLock::new(RawStoreConfig::default()))
        }
    }
}

// TODO no unwrap
impl StoreConfig {
    pub fn load_or_default() -> StoreConfig {
        let res = StoreConfig::load(&default_config_path());
        
        let config = match res {
            Ok(v) => v,
            Err(_) => StoreConfig::default()
        };

        if !config.cache_path().exists() {
            std::fs::create_dir_all(&*config.cache_path()).unwrap();
        }

        config
    }

    pub fn load(config_path: &Path) -> io::Result<StoreConfig> {
        let file = File::open(config_path)?;
        let data: RawStoreConfig = serde_json::from_reader(file)?;

        Ok(StoreConfig {
            config_path: config_path.to_owned(),
            data: Arc::new(RwLock::new(data))
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

    pub fn skipped_package(&self, key: &AbsolutePackageKey) -> Option<String> {
        self.data.read().unwrap().skipped_packages.get(key).map(|x| x.to_string())
    }

    pub fn remove_skipped_package(&self, key: &AbsolutePackageKey) -> Result<(), ()> {
        self.data.write().unwrap().skipped_packages.remove(key);
        self.save()
    }

    pub fn add_skipped_package(&self, key: AbsolutePackageKey, version: String) -> Result<(), ()> {
        self.data.write().unwrap().skipped_packages.insert(key, version);
        self.save()
    }

    pub fn cache_path(&self) -> PathBuf {
        self.data.read().unwrap().cache_path.clone()
    }

    pub fn set_cache_path(&self, cache_path: PathBuf) -> Result<(), ()> {
        self.data.write().unwrap().cache_path = cache_path;
        self.save()
    }

    pub fn repos(&self) -> Vec<RepoRecord> {
        self.data.read().unwrap().repos.clone()
    }

    pub fn add_repo(&self, repo_record: RepoRecord) -> Result<(), ()> {
        self.data.write().unwrap().repos.push(repo_record);
        self.save()
    }

    pub fn remove_repo(&self, repo_record: RepoRecord) -> Result<(), ()> {
        match self.data.read().unwrap().repos.iter().position(|r| r == &repo_record) {
            Some(index) => {
                self.data.write().unwrap().repos.remove(index);
                self.save()
            },
            None => Ok(())
        }
    }

    pub fn update_repo(&self, index: usize, repo_record: RepoRecord) -> Result<(), ()> {
        self.data.write().unwrap().repos[index] = repo_record;
        self.save()
    }
}

#[derive(Debug, Clone, Copy)]
pub enum PackageDependencyError {
    PackageNotFound,
    VersionNotFound
}

#[derive(Debug)]
pub struct PackageDependency {
    pub id: AbsolutePackageKey,
    pub version: String,
    pub level: u8
}
