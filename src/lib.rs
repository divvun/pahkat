extern crate pahkat;
extern crate rusqlite;
extern crate reqwest;
#[macro_use]
extern crate serde_json;
extern crate serde;
#[macro_use]
extern crate serde_derive;
extern crate semver;
extern crate xz2;
extern crate tar;
extern crate tempdir;
extern crate url;
extern crate rhai;
#[cfg(windows)]
extern crate winreg;
#[cfg(feature = "ipc")]
extern crate jsonrpc_core;
#[cfg(feature = "ipc")]
extern crate jsonrpc_pubsub;
#[macro_use]
#[cfg(feature = "ipc")]
extern crate jsonrpc_macros;
#[cfg(feature = "ipc")]
extern crate jsonrpc_tcp_server;
#[cfg(target_os = "macos")]
extern crate plist;
extern crate crypto;

use std::env;

use crypto::digest::Digest;
use crypto::sha2::Sha256;
use pahkat::types::*;
use pahkat::types::{Repository as RepositoryMeta};
use pahkat::types::Downloadable;
use std::io::{self, BufWriter, Read, Write};
use std::path::{Path, PathBuf};
use std::fs::{remove_file, read_dir, remove_dir, create_dir_all, File};
use std::fmt;

#[cfg(windows)]
mod windows;
#[cfg(target_os = "macos")]
pub mod macos;
#[cfg(feature = "ipc")]
pub mod ipc;
pub mod tarball;

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

    pub fn load_default() -> Result<StoreConfig, ()> {
        return StoreConfig::load(&env::home_dir().unwrap()
            .join("Library/Application Support/Pahkat/config.json"))
    }

    pub fn load(config_path: &Path) -> Result<StoreConfig, ()> {
        if !config_path.exists() {
            return Err(())
        }

        let file = File::open(config_path).unwrap();
        let config: StoreConfig = serde_json::from_reader(file).unwrap();

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
    fn download<F>(&self, dir_path: &Path, progress: Option<F>) -> Option<PathBuf>
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

impl Download for Package {

    // TODO: should return Result<PathBuf, E>
    fn download<F>(&self, dir_path: &Path, progress: Option<F>) -> Option<PathBuf>
            where F: Fn(usize, usize) -> () {
        use reqwest::header::*;

        let installer = match self.installer() {
            Some(v) => v,
            None => return None
        };
        let url_str = installer.url();

        let url = url::Url::parse(&url_str).unwrap();
        let mut res = reqwest::get(&url_str).unwrap();

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
            return None;
        }

        Some(tmp_path)
    }
}

#[derive(Debug)]
pub enum RepoDownloadError {
    ReqwestError(reqwest::Error),
    JsonError(serde_json::Error)
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

