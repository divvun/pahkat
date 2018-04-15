extern crate pahkat;
#[cfg(prefix)]
extern crate rusqlite;
extern crate reqwest;
extern crate serde_json;
extern crate serde;
#[macro_use]
extern crate serde_derive;
#[macro_use]
extern crate lazy_static;
extern crate semver;
extern crate tempdir;
extern crate url;

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
#[macro_use]
#[cfg(target_os = "macos")]
extern crate maplit;

#[cfg(windows)]
extern crate winapi;

extern crate crypto;
extern crate sentry;

use std::env;
use sentry::sentry::Sentry;
use crypto::digest::Digest;
use crypto::sha2::Sha256;
use pahkat::types::*;
use pahkat::types::{Repository as RepositoryMeta};
use pahkat::types::Downloadable;
use std::io::{self, BufWriter, Write};
use std::path::{Path, PathBuf};
use std::fs::{create_dir_all, File};
use std::fmt;
// use std::sync::{Arc, Mutex};

#[cfg(windows)]
pub mod windows;
#[cfg(target_os = "macos")]
pub mod macos;
#[cfg(feature = "ipc")]
pub mod ipc;
pub mod tarball;

const DSN: &'static str = "https://0a0fc86e9d2447e8b0b807087575e8c6:3d610a0fea7b49d6803061efa16c2ddc@sentry.io/301711";

lazy_static! {
    static ref SENTRY: Sentry = Sentry::new(&DSN).unwrap();
}

// pub mod exports;

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

// pub trait PackageStore<'a> {
//     type InstallResult;
//     type UninstallResult;
//     type StatusResult;
//     type InstallContext;
//     type UninstallContext;

//     fn install(&self, package: &'a Package, context: &'a Self::InstallContext) -> Self::InstallResult;
//     fn uninstall(&self, package: &'a Package, context: &'a Self::UninstallContext) -> Self::UninstallResult;
//     fn status(&self, package: &'a Package) -> Self::StatusResult;
// }


#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct StoreConfig {
    pub url: String,
    pub cache_dir: String
}

#[cfg(target_os = "macos")]
pub fn default_config_path() -> PathBuf {
    env::home_dir().unwrap().join("Library/Application Support/Pahkat/config.json")
}

#[cfg(target_os = "linux")]
pub fn default_config_path() -> PathBuf {
    env::home_dir().unwrap().join(".config/pahkat/config.json")
}

#[cfg(windows)]
pub fn default_config_path() -> PathBuf {
    env::home_dir().unwrap().join(r#"AppData\Roaming\Pahkat\config.json"#)
}

#[cfg(target_os = "macos")]
pub fn default_cache_path() -> PathBuf {
    env::home_dir().unwrap().join("Library/Caches/Pahkat/packages")
}

#[cfg(target_os = "linux")]
pub fn default_cache_path() -> PathBuf {
    env::home_dir().unwrap().join(".cache/pahkat/packages")
}

#[cfg(windows)]
pub fn default_cache_path() -> PathBuf {
    env::home_dir().unwrap().join(r#"AppData\Local\Pahkat\packages"#)
}


impl StoreConfig {
    pub fn save(&self, config_path: &Path) -> Result<(), ()> { 
        let cfg_str = serde_json::to_string_pretty(&self).unwrap();
        {
            create_dir_all(config_path.parent().unwrap()).unwrap();
            let mut file = File::create(config_path).unwrap();
            file.write_all(cfg_str.as_bytes()).unwrap();
        }

        Ok(())
    }

    pub fn load_or_default() -> StoreConfig {
        let res = StoreConfig::load(&default_config_path());
        
        let config = match res {
            Ok(v) => v,
            Err(_) => StoreConfig {
                url: "".to_owned(),
                cache_dir: default_cache_path()
                    .to_str().unwrap().to_owned()
            }
        };

        if Path::new(&config.cache_dir).exists() {
            create_dir_all(&config.cache_dir).unwrap();
        }

        config
    }

    pub fn load(config_path: &Path) -> io::Result<StoreConfig> {
        let file = File::open(config_path)?;
        let config: StoreConfig = serde_json::from_reader(file)?;

        Ok(config)
    }
}

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Repository {
    meta: RepositoryMeta,
    packages: Packages,
    virtuals: Virtuals
}

impl Repository {
    pub fn from_url(url: &str) -> Result<Repository, RepoDownloadError> {
        download_repository(url)
    }

    pub fn package(&self, key: &str) -> Option<&Package> {
        let map = &self.packages.packages;
        map.get(key)
    }

    pub fn packages(&self) -> &PackageMap {
        &self.packages.packages
    }

    pub fn virtuals(&self) -> &VirtualRefMap {
        &self.virtuals.virtuals
    }

    pub fn hash_id(&self) -> String {
        let mut sha = Sha256::new();
        sha.input_str(&self.meta.base);
        sha.result_str()
    }
}

pub trait Download {
    fn download<F>(&self, dir_path: &Path, progress: Option<F>) -> Result<PathBuf, DownloadError>
            where F: Fn(usize, usize) -> ();
}

struct ProgressWriter<W: Write, F>
    where F: Fn(usize, usize) -> ()
{
    writer: W,
    callback: F,
    max_count: usize,
    cur_count: usize
}

impl<W: Write, F> ProgressWriter<W, F>
    where F: Fn(usize, usize) -> ()
{
    fn new(writer: W, max_count: usize, callback: F) -> ProgressWriter<W, F> {
        (callback)(0, max_count);

        ProgressWriter {
            writer: writer,
            callback: callback,
            max_count: max_count,
            cur_count: 0
        }
    }
}

impl<W: Write, F> Write for ProgressWriter<W, F>
    where F: Fn(usize, usize) -> ()
{
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        use std::cmp;
        
        let new_count = self.cur_count + buf.len();
        self.cur_count = cmp::min(new_count, self.max_count);
        (self.callback)(self.cur_count, self.max_count);
        self.writer.write(buf)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.writer.flush()
    }
}

#[derive(Debug)]
pub enum DownloadError {
    EmptyFile,
    NoUrl,
    ReqwestError(reqwest::Error),
    HttpStatusFailure(u16)
}

impl Download for Package {
    fn download<F>(&self, dir_path: &Path, progress: Option<F>) -> Result<PathBuf, DownloadError>
            where F: Fn(usize, usize) -> () {
        use reqwest::header::*;

        let installer = match self.installer() {
            Some(v) => v,
            None => return Err(DownloadError::NoUrl)
        };
        let url_str = installer.url();

        let url = url::Url::parse(&url_str).unwrap();
        let mut res = match reqwest::get(&url_str) {
            Ok(v) => v,
            Err(e) => return Err(DownloadError::ReqwestError(e))
        };

        if !res.status().is_success() {
            return Err(DownloadError::HttpStatusFailure(res.status().as_u16()))
        }

        let filename = &url.path_segments().unwrap().last().unwrap();
        let tmp_path = dir_path.join(&filename).to_path_buf();
        let file = File::create(&tmp_path).unwrap();
    
        let mut buf_writer = BufWriter::new(file);

        let write_res = match progress {
            Some(cb) => {
                let len = {
                    res.headers().get::<ContentLength>()
                        .map(|ct_len| **ct_len as usize)
                        .unwrap_or(0usize)
                };
                res.copy_to(&mut ProgressWriter::new(buf_writer, len, cb))
            },
            None => res.copy_to(&mut buf_writer)
        };
        
        if write_res.unwrap() == 0 {
            return Err(DownloadError::EmptyFile);
        }

        Ok(tmp_path)
    }
}

#[derive(Debug)]
pub enum RepoDownloadError {
    ReqwestError(reqwest::Error),
    JsonError(serde_json::Error)
}

impl fmt::Display for RepoDownloadError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match *self {
            RepoDownloadError::ReqwestError(ref e) => e.fmt(f),
            RepoDownloadError::JsonError(ref e) => e.fmt(f)
        }
    }
}

pub fn download_repository(url: &str) -> Result<Repository, RepoDownloadError> {
    let client = reqwest::Client::new();

    let mut meta_res = client.get(&format!("{}/index.json", url)).send()
        .map_err(|e| RepoDownloadError::ReqwestError(e))?;
    let meta_text = meta_res.text()
        .map_err(|e| RepoDownloadError::ReqwestError(e))?;
    let meta: RepositoryMeta = serde_json::from_str(&meta_text)
        .map_err(|e| RepoDownloadError::JsonError(e))?;

    let mut pkg_res = client.get(&format!("{}/packages/index.json", url)).send()
        .map_err(|e| RepoDownloadError::ReqwestError(e))?;
    let pkg_text = pkg_res.text()
        .map_err(|e| RepoDownloadError::ReqwestError(e))?;
    let packages: Packages = serde_json::from_str(&pkg_text)
        .map_err(|e| RepoDownloadError::JsonError(e))?;

    let mut virt_res = client.get(&format!("{}/virtuals/index.json", url)).send()
        .map_err(|e| RepoDownloadError::ReqwestError(e))?;
    let virt_text = virt_res.text()
        .map_err(|e| RepoDownloadError::ReqwestError(e))?;
    let virtuals: Virtuals = serde_json::from_str(&virt_text)
        .map_err(|e| RepoDownloadError::JsonError(e))?;

    Ok(Repository {
        meta: meta,
        packages: packages,
        virtuals: virtuals
    })
}

