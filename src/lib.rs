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

use pahkat::types::*;
use pahkat::types::{Repository as RepositoryMeta};
use pahkat::types::Downloadable;
use std::io::{self, BufWriter, Read, Write};
use std::path::{Path, PathBuf};
use std::fs::{remove_file, read_dir, remove_dir, create_dir_all, File};

#[cfg(windows)]
mod windows;
#[cfg(target_os = "macos")]
mod macos;
#[cfg(feature = "ipc")]
pub mod ipc;
pub mod tarball;

pub mod exports;

pub enum PackageStatus {
    NotInstalled,
    UpToDate,
    RequiresUpdate
}

pub enum PackageStatusError {
    NoInstaller,
    WrongInstallerType,
    ParsingVersion,
    InvalidInstallPath,
    InvalidMetadata
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

#[derive(Debug, Serialize, Deserialize)]
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
}

pub trait Download {
    fn download(&self, dir_path: &Path) -> Option<PathBuf>;
}

impl Download for Package {
    fn download(&self, dir_path: &Path) -> Option<PathBuf> {
        let installer = match self.installer() {
            Some(v) => v,
            None => return None
        };
        let url_str = installer.url();

        // TODO: this should write directly to the file
        // TODO: this should use a provided cache dir
        let url = url::Url::parse(&url_str).unwrap();
        let mut res = reqwest::get(&url_str).unwrap();
        let tmppath = dir_path.join(&url.path_segments().unwrap().last().unwrap()).to_path_buf();
        let file = File::create(&tmppath).unwrap();
        
        let mut writer = BufWriter::new(file);
        if res.copy_to(&mut writer).unwrap() == 0 {
            return None;
        }

        Some(tmppath)
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

