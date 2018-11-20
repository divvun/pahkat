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
extern crate url;
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

#[cfg(windows)]
pub mod windows;
#[cfg(target_os = "macos")]
pub mod macos;
pub mod tarball;

mod download;
mod repo;
pub use self::download::Download;
pub use self::repo::Repository;

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum PackageStatus {
    NotInstalled,
    UpToDate,
    RequiresUpdate
}

impl fmt::Display for PackageStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(f, "{}", match *self {
            PackageStatus::NotInstalled => "Not installed",
            PackageStatus::UpToDate => "Up to date",
            PackageStatus::RequiresUpdate => "Requires update"
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

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
struct RawStoreConfig {
    #[serde(default = "Vec::new")]
    pub repo_urls: Vec<String>,
    #[serde(default = "default_cache_path")]
    pub cache_path: PathBuf
}

impl std::default::Default for RawStoreConfig {
    fn default() -> RawStoreConfig {
        RawStoreConfig {
            repo_urls: vec![],
            cache_path: default_cache_path()
        }
    }
}

#[derive(Debug, Clone)]
pub struct StoreConfig {
    /// A reference to the path for this StoreConfig
    config_path: PathBuf,
    data: RefCell<RawStoreConfig>
}

impl std::default::Default for StoreConfig {
    fn default() -> StoreConfig {
        StoreConfig {
            config_path: default_config_path(),
            data: RefCell::new(RawStoreConfig::default())
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
            data: RefCell::new(data)
        })
    }

    pub fn save(&self) -> Result<(), ()> { 
        let cfg_str = serde_json::to_string_pretty(&self.data).unwrap();
        {
            create_dir_all(self.config_path.parent().unwrap()).unwrap();
            let mut file = File::create(&self.config_path).unwrap();
            file.write_all(cfg_str.as_bytes()).unwrap();
        }

        Ok(())
    }

    pub fn cache_path(&self) -> Ref<PathBuf> {
        Ref::map(self.data.borrow(), |x| &x.cache_path)
    }

    pub fn set_cache_path(&self, cache_path: PathBuf) {
        self.data.borrow_mut().cache_path = cache_path;
        self.save();
    }

    pub fn repo_urls(&self) -> Ref<Vec<String>> {
        Ref::map(self.data.borrow(), |x| &x.repo_urls)
    }

    pub fn add_repo_url(&self, repo_url: String) {
        self.data.borrow_mut().repo_urls.push(repo_url);
        self.save();
    }

    pub fn remove_repo_url(&self, repo_url: &str) {
        match self.data.borrow().repo_urls.iter().position(|r| r == repo_url) {
            Some(index) => {
                self.data.borrow_mut().repo_urls.remove(index);
                self.save();
            },
            None => {}
        }
    }
}
