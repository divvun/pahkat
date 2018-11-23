use pahkat::types::{
    MacOSInstallTarget,
    MacOSInstaller,
    Installer,
    Package,
    Downloadable
};
use std::path::{Path, PathBuf};
use std::fs::{remove_file, remove_dir};
use std::fmt::Display;
use std::str::FromStr;
use std::process;
use std::process::Command;
use std::collections::{HashMap, BTreeMap};
use crate::repo::{Repository, PackageRecord};
use dirs;
use crypto::digest::Digest;
use crypto::sha2::Sha256;

use serde::de::{self, Deserialize, Deserializer};
use plist::serde::{deserialize as deserialize_plist};

use crate::{RepoRecord};
use crate::*;

fn from_str<'de, T, D>(deserializer: D) -> Result<T, D::Error>
    where T: FromStr,
          T::Err: Display,
          D: Deserializer<'de>
{
    let s = String::deserialize(deserializer)?;
    T::from_str(&s).map_err(de::Error::custom)
}

#[derive(Debug, Deserialize)]
struct BundlePlistInfo {
    #[serde(rename = "CFBundleIdentifier")]
    pub identifier: Option<String>,
    #[serde(rename = "CFBundleName")]
    pub name: Option<String>,
    #[serde(default, deserialize_with = "from_str", rename = "CFBundleVersion")]
    pub version: usize,
    #[serde(rename = "CFBundleShortVersionString")]
    pub short_version: Option<String>,
}

#[test]
fn test_bundle_plist() {
    let file = File::open("/Users/Brendan/Library/Keyboard Layouts/so.brendan.keyboards.keyboardlayout.brendan.bundle/Contents/Info.plist").unwrap();
    let plist: BundlePlistInfo = deserialize_plist(file).unwrap();
    println!("{:?}", plist);
}

#[derive(Debug)]
pub enum MacOSInstallError {
    NoInstaller,
    WrongInstallerType,
    InvalidFileType,
    PackageNotInCache,
    InvalidUrl(String),
    InstallerFailure(ProcessError)
}

#[derive(Debug)]
pub enum MacOSUninstallError {
    NoInstaller,
    WrongInstallerType,
    PkgutilFailure(ProcessError)
}

pub struct MacOSPackageStore {
    repos: RefCell<HashMap<RepoRecord, Repository>>,
    config: StoreConfig
}

impl MacOSPackageStore {
    pub fn new(config: StoreConfig) -> MacOSPackageStore {
        let store = MacOSPackageStore {
            repos: RefCell::new(HashMap::new()),
            config
        };

        store.refresh_repos();

        store
    }

    fn download_path(&self, url: &str) -> PathBuf {
        let mut sha = Sha256::new();
        sha.input_str(url);
        let hash_id = sha.result_str();
        
        self.config.cache_path().join(hash_id)
    }

    pub fn refresh_repos(&self) {
        let mut repos = HashMap::new();

        for record in self.config.repos().iter() {
            if let Ok(repo) = Repository::from_url(&record.url) {
                repos.insert(record.clone(), repo);
            }
        }

        *self.repos.borrow_mut() = repos;
    }

    pub fn config(&self) -> &StoreConfig {
        &self.config
    }

    pub fn add_repo(&self, url: String, channel: String) -> Result<(), ()> {
        self.config().add_repo(RepoRecord { url, channel })?;
        self.refresh_repos();
        Ok(())
    }

    pub fn remove_repo(&self, url: String, channel: String) -> Result<(), ()> {
        self.config().remove_repo(RepoRecord { url, channel })?;
        self.refresh_repos();
        Ok(())
    }

    pub fn update_repo(&self, index: usize, url: String, channel: String) -> Result<(), ()> {
        self.config().update_repo(index, RepoRecord { url, channel })?;
        self.refresh_repos();
        Ok(())
    }

    pub fn find_package(&self, package_id: &str) -> Option<PackageRecord> {
        self.repos.borrow().iter()
            .find_map(|(key, repo)| {
                repo.packages()
                    .get(package_id)
                    .map(|x| PackageRecord::new(repo.meta(), &key.channel, x.to_owned()))
            })
    }

    pub fn download<F>(&self, record: &PackageRecord, progress: Option<F>) -> Result<PathBuf, crate::download::DownloadError>
            where F: Fn(u64, u64) -> () {
        let installer = match record.package().installer() {
            None => return Err(crate::download::DownloadError::NoUrl),
            Some(v) => v
        };

        record.package().download(&self.download_path(&installer.url()), progress)
    }

    pub fn install(&self, record: &PackageRecord, target: MacOSInstallTarget) -> Result<PackageStatus, MacOSInstallError> {
        let installer = match record.package().installer() {
            None => return Err(MacOSInstallError::NoInstaller),
            Some(v) => v
        };

        let installer = match *installer {
            Installer::MacOS(ref v) => v,
            _ => return Err(MacOSInstallError::WrongInstallerType)
        };

        let url = url::Url::parse(&installer.url)
            .map_err(|_| MacOSInstallError::InvalidUrl(installer.url.to_owned()))?;
        let filename = url.path_segments().unwrap().last().unwrap();
        let pkg_path = self.download_path(&url.as_str()).join(filename);

        if !pkg_path.exists() {
            return Err(MacOSInstallError::PackageNotInCache)
        }
        
        match install_macos_package(&pkg_path, target) {
            Err(e) => return Err(MacOSInstallError::InstallerFailure(e)),
            _ => {}
        };

        Ok(self.status_impl(&installer, record, target).unwrap())
    }

    pub fn uninstall(&self, record: &PackageRecord, target: MacOSInstallTarget) -> Result<PackageStatus, MacOSUninstallError> {
        let installer = match record.package().installer() {
            None => return Err(MacOSUninstallError::NoInstaller),
            Some(v) => v
        };

        let installer = match installer {
            &Installer::MacOS(ref v) => v,
            _ => return Err(MacOSUninstallError::WrongInstallerType)
        };

        match uninstall_macos_package(&installer.pkg_id, target) {
            Err(e) => return Err(MacOSUninstallError::PkgutilFailure(e)),
            _ => {}
        };

        Ok(self.status_impl(installer, record, target).unwrap())
    }

    pub fn status(&self, record: &PackageRecord, target: MacOSInstallTarget) -> Result<PackageStatus, PackageStatusError> {
        let installer = match record.package().installer() {
            None => return Err(PackageStatusError::NoInstaller),
            Some(v) => v
        };

        let installer = match installer {
            &Installer::MacOS(ref v) => v,
            _ => return Err(PackageStatusError::WrongInstallerType)
        };

        self.status_impl(installer, record, target)
    }

    fn status_impl(&self, installer: &MacOSInstaller, record: &PackageRecord, target: MacOSInstallTarget) -> Result<PackageStatus, PackageStatusError> {
        let pkg_info = match get_package_info(&installer.pkg_id, target) {
            Ok(v) => v,
            Err(e) => {
                match e {
                    ProcessError::NotFound => {},
                    _ => {
                        eprintln!("{:?}", e); 
                    }
                };
                
                return Ok(PackageStatus::NotInstalled);
            }
        };

        let installed_version = match semver::Version::parse(&pkg_info.pkg_version) {
            Err(_) => return Err(PackageStatusError::ParsingVersion),
            Ok(v) => v
        };

        let candidate_version = match semver::Version::parse(&record.package().version) {
            Err(_) => return Err(PackageStatusError::ParsingVersion),
            Ok(v) => v
        };

        // TODO: handle skipped versions
        if let Some(skipped_version) = self.config().skipped_package(record.id()) {
            match semver::Version::parse(&skipped_version) {
                Err(_) => {}, // No point giving up now
                Ok(v) => {
                    if candidate_version <= v {
                        return Ok(PackageStatus::Skipped)
                    }
                }
            }
        }

        if candidate_version > installed_version {
            Ok(PackageStatus::RequiresUpdate)
        } else {
            Ok(PackageStatus::UpToDate)
        }
    }
}

#[derive(Debug, Deserialize)]
struct MacOSPackageExportPath {
    pub gid: u64,
    #[serde(rename = "install-time")]
    pub install_time: u64,
    pub mode: u64,
    #[serde(rename = "pkg-version")]
    pub pkg_version: String,
    pub pkgid: String,
    pub uid: u64
}

#[derive(Debug, Deserialize)]
struct MacOSPackageExportPlist {
    #[serde(rename = "install-location")]
    pub install_location: String,
    #[serde(rename = "install-time")]
    pub install_time: u64,
    pub paths: BTreeMap<String, MacOSPackageExportPath>,
    #[serde(rename = "pkg-version")]
    pub pkg_version: String,
    pub pkgid: String,
    #[serde(rename = "receipt-plist-version")]
    pub receipt_plist_version: f64,
    pub volume: String
}

impl MacOSPackageExportPlist {
    fn path(&self) -> PathBuf {
        Path::new(&self.volume).join(&self.install_location)
    }

    fn paths(&self) -> Vec<PathBuf> {
        let base_path = self.path();
        self.paths.keys().map(|p| base_path.join(p)).collect()
    }
}

fn get_package_info(bundle_id: &str, target: MacOSInstallTarget) -> Result<MacOSPackageExportPlist, ProcessError> {
    use std::io::Cursor;

    let home_dir = dirs::home_dir().expect("Always find home directory");
    let mut args = vec!["--export-plist", bundle_id];
    if let MacOSInstallTarget::User = target {
        args.push("--volume");
        args.push(&home_dir.to_str().unwrap());
    }
    let res = Command::new("pkgutil").args(&args).output();
    let output = match res {
        Ok(v) => v,
        Err(e) => {
            return Err(ProcessError::Io(e));
        }
    };

    if !output.status.success() {
        if let Some(code) = output.status.code() {
            if code == 1 {
              return Err(ProcessError::NotFound);
            }
        }
        
        return Err(ProcessError::Unknown(output));
    }

    let plist_data = String::from_utf8(output.stdout).expect("plist should always be valid UTF-8");
    let cursor = Cursor::new(plist_data);
    let plist: MacOSPackageExportPlist = deserialize_plist(cursor).expect("plist should always be valid");
    return Ok(plist);
}

#[derive(Debug)]
pub enum ProcessError {
    Io(io::Error),
    Unknown(process::Output),
    NotFound
}

fn install_macos_package(pkg_path: &Path, target: MacOSInstallTarget) -> Result<(), ProcessError> {
    let target_str = match target {
        MacOSInstallTarget::User => "CurrentUserHomeDirectory",
        MacOSInstallTarget::System => "LocalSystem"
    };

    let args = &[
        "-pkg",
        &pkg_path.to_str().unwrap(),
        "-target",
        target_str
    ];

    let res = Command::new("installer").args(args).output();
    let output = match res {
        Ok(v) => v,
        Err(e) => return Err(ProcessError::Io(e))
    };
    if !output.status.success() {
        let _msg = format!("Exit code: {}", output.status.code().unwrap());
        return Err(ProcessError::Unknown(output));
    }
    Ok(())
}

fn uninstall_macos_package(bundle_id: &str, target: MacOSInstallTarget) -> Result<(), ProcessError> {
    let package_info = get_package_info(bundle_id, target)?;

    let mut errors = vec![];
    let mut directories = vec![];

    for path in package_info.paths() {
        let meta = match path.symlink_metadata() {
            Ok(v) => v,
            Err(err) => { errors.push(err); continue; }
        };

        if meta.is_dir() {
            directories.push(path);
            continue;
        }

        eprintln!("Deleting: {:?}", &path);
        match remove_file(&path) {
            Err(err) => {
                match err.kind() {
                    io::ErrorKind::NotFound => {},
                    _ => {
                        eprintln!("{:?}: {:?}", &path, &err);
                        errors.push(err);
                    }
                }
            },
            Ok(_) => {}
        };
    }

    // Ensure children are deleted first
    directories.sort_unstable_by(|a, b| {
        let a_count = a.to_string_lossy().chars().filter(|x| *x == '/').count();
        let b_count = b.to_string_lossy().chars().filter(|x| *x == '/').count();
        b_count.cmp(&a_count)
    });

    for dir in directories {
        eprintln!("Deleting: {:?}", &dir);
        match remove_dir(&dir) {
            Err(err) => {
                eprintln!("{:?}: {:?}", &dir, &err);
                errors.push(err);
            }
            Ok(_) => {}
        }
    }

    eprintln!("{:?}", errors);
    
    forget_pkg_id(bundle_id, target)?;

    Ok(())
}

fn forget_pkg_id(bundle_id: &str, target: MacOSInstallTarget) -> Result<(), ProcessError> {
    let home_dir = dirs::home_dir().expect("Always find home directory");
    let mut args = vec!["--forget", bundle_id];
    if let MacOSInstallTarget::User = target {
        args.push("--volume");
        args.push(&home_dir.to_str().unwrap());
    }

    let res = Command::new("pkgutil").args(&args).output();
    let output = match res {
        Ok(v) => v,
        Err(e) => {
            
            eprintln!("{:?}", e);
            return Err(ProcessError::Io(e));
        }
    };
    if !output.status.success() {
        eprintln!("{:?}", output.status.code().unwrap());
        return Err(ProcessError::Unknown(output));
    }
    Ok(())
}
