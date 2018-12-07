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
use std::sync::{Arc, RwLock};
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread::{self, JoinHandle};

// use crossbeam::channel;

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

// struct UiBindings {
//     fn list_repos_json() -> String {
//         unimplemented!();
//     }

//     fn all_repo_data_json() -> String {
//         unimplemented();
//     }
// }

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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
}

#[derive(Debug, Clone)]
pub struct PackageAction {
    pub package: PackageRecord,
    pub action: PackageActionType,
    pub target: MacOSInstallTarget
}

pub struct TransactionDisposable {
    is_cancelled: Arc<AtomicBool>,
    // result: Option<Result<PathBuf, DownloadError>>,
    // handle: Option<JoinHandle<Result<PathBuf, DownloadError>>>
}

// impl TransactionDisposable {
//     fn cancel(&self) {

//     }

//     fn wait(&self) {

//     }
// }

pub struct PackageTransaction {
    store: Arc<MacOSPackageStore>,
    actions: Arc<Vec<PackageAction>>,
    is_cancelled: Arc<AtomicBool>
}

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

impl PackageTransaction {
    pub fn new(
        store: Arc<MacOSPackageStore>,
        actions: Vec<PackageAction>
    ) -> PackageTransaction {
        PackageTransaction {
            store,
            actions: Arc::new(actions),
            is_cancelled: Arc::new(AtomicBool::new(false))
        }
    }

    pub fn validate(&self) -> bool {
        true
    }

    // pub fn download<F>(&mut self, progress: F) where F: Fn(u64, u64) -> () {
    //     if !self.validate() {
    //         // TODO: early return
    //         return;
    //     }

    //     let is_cancelled = self.is_cancelled.clone();
    //     let store = self.store.clone();
    //     let actions = self.actions.clone();

    //     let handle = std::thread::spawn(move || {
    //         for action in actions.iter().filter(|a| a.action == PackageActionType::Install) {
    //             if is_cancelled.load(Ordering::Relaxed) == true {
    //                 return;
    //             }
                
                
    //         }

    //         ()
    //     });
    // }

    pub fn process<F>(&mut self, progress: F)
    where F: Fn(AbsolutePackageKey, TransactionEvent) -> () + 'static + Send {
        if !self.validate() {
            // TODO: early return
            return;
        }

        let is_cancelled = self.is_cancelled.clone();
        let store = self.store.clone();
        let actions = self.actions.clone();

        let handle = std::thread::spawn(move || {
            for action in actions.iter() {
                if is_cancelled.load(Ordering::Relaxed) == true {
                    return;
                }

                match action.action {
                    PackageActionType::Install => {
                        progress(action.package.id().clone(), TransactionEvent::Installing);
                        match store.install(&action.package, action.target) {
                            Ok(_) => progress(action.package.id().clone(), TransactionEvent::Completed),
                            Err(_) => progress(action.package.id().clone(), TransactionEvent::Error)
                        };
                    },
                    PackageActionType::Uninstall => {
                        progress(action.package.id().clone(), TransactionEvent::Uninstalling);
                        match store.uninstall(&action.package, action.target) {
                            Ok(_) => progress(action.package.id().clone(), TransactionEvent::Completed),
                            Err(_) => progress(action.package.id().clone(), TransactionEvent::Error)
                        };
                    }
                }
            }

            ()
        });

        handle.join();
    }

    pub fn cancel(&self) -> bool {
        // let prev_value = *self.is_cancelled.read().unwrap();
        // *self.is_cancelled.write().unwrap() = true;
        // prev_value
        unimplemented!()
    }
}


pub struct MacOSPackageStore {
    repos: Arc<RwLock<HashMap<RepoRecord, Repository>>>,
    config: Arc<RwLock<StoreConfig>>
}

impl MacOSPackageStore {
    pub fn new(config: StoreConfig) -> MacOSPackageStore {
        let store = MacOSPackageStore {
            repos: Arc::new(RwLock::new(HashMap::new())),
            config: Arc::new(RwLock::new(config))
        };

        store.refresh_repos();

        store
    }

    pub fn config(&self) -> StoreConfig {
        self.config.read().unwrap().clone()
    }

    pub(crate) fn repos_json(&self) -> String {
        serde_json::to_string(&self.repos.read().unwrap().values().collect::<Vec<_>>()).unwrap()
    }

    pub fn refresh_repos(&self) {
        let mut repos = HashMap::new();
        let config = self.config.read().unwrap();
        for record in config.repos().iter() {
            match Repository::from_cache_or_url(
                &record.url,
                record.channel.clone(),
                &config.repo_cache_path()
            ) {
                Ok(repo) => { repos.insert(record.clone(), repo); },
                Err(e) => { println!("{:?}", e); }
            };
        }

        *self.repos.write().unwrap() = repos;
    }

    pub fn add_repo(&self, url: String, channel: String) -> Result<(), ()> {
        self.config().add_repo(RepoRecord { url: Url::parse(&url).unwrap(), channel })?;
        self.refresh_repos();
        Ok(())
    }

    pub fn remove_repo(&self, url: String, channel: String) -> Result<(), ()> {
        self.config().remove_repo(RepoRecord { url: Url::parse(&url).unwrap(), channel })?;
        self.refresh_repos();
        Ok(())
    }

    pub fn update_repo(&self, index: usize, url: String, channel: String) -> Result<(), ()> {
        self.config().update_repo(index, RepoRecord { url: Url::parse(&url).unwrap(), channel })?;
        self.refresh_repos();
        Ok(())
    }

    pub fn resolve_package(&self, package_key: &AbsolutePackageKey) -> Option<PackageRecord> {
        println!("Resolving package: url: {}, channel: {}", &package_key.url, &package_key.channel);
        for k in self.repos.read().unwrap().keys() {
            println!("{:?}", k);
        }

        self.repos.read().unwrap()
            .get(&RepoRecord {
                url: package_key.url.clone(),
                channel: package_key.channel.clone()
            })
            .and_then(|r| {
                println!("Got repo: {:?}", r);
                for k in r.packages().keys() {
                    println!("Pkg id: {}, {}", &k, k == &package_key.id);
                }

                println!("My pkg id: {}", &package_key.id);
                let pkg = match r.packages().get(&package_key.id) {
                    Some(x) => Some(PackageRecord::new(r.meta(), &package_key.channel, x.to_owned())),
                    None => None
                };
                println!("Found pkg: {:?}", &pkg);
                pkg
            })
    }

    pub fn find_package(&self, package_id: &str) -> Option<PackageRecord> {
        self.repos.read().unwrap().iter()
            .find_map(|(key, repo)| {
                repo.packages()
                    .get(package_id)
                    .map(|x| PackageRecord::new(repo.meta(), &key.channel, x.to_owned()))
            })
    }

    /// Get the dependencies for a given package
    pub fn find_package_dependencies(&self, record: &PackageRecord, target: MacOSInstallTarget) -> Result<Vec<PackageDependency>, PackageDependencyError> {
        let mut resolved = Vec::<String>::new();
        Ok(self.find_package_dependencies_impl(record, target, 0, &mut resolved)?)
    }

    fn find_package_dependencies_impl(
        &self, record: &PackageRecord,
        target: MacOSInstallTarget,
        level: u8,
        resolved: &mut Vec<String>) -> Result<Vec<PackageDependency>, PackageDependencyError> {

        let mut result = Vec::<PackageDependency>::new();

        fn push_if_not_exists(dependency: PackageDependency, result: &mut Vec<PackageDependency>) {
            if result.iter().filter(|d| d.id == dependency.id).count() == 0 {
                result.push(dependency);
            }
        }

        for (package_id, version) in record.package().dependencies.iter() {
            // avoid circular references by keeping
            // track of package ids that have already been processed
            if resolved.contains(package_id) {
                continue;
            }
            resolved.push(package_id.clone());

            match self.find_package(package_id.as_str()) {
                Some(ref dependency_record) => {
                    // add all the dependencies of the dependency
                    // to the list result first
                    for dependency in self.find_package_dependencies_impl(dependency_record, target, level + 1, resolved)? {
                        push_if_not_exists(dependency, &mut result);
                    }

                    // make sure the version requirement is correct
                    if dependency_record.package().version.as_str() != version {
                        return Err(PackageDependencyError::VersionNotFound);
                    }

                    match self.status(dependency_record, target) {
                        Err(error) => return Err(PackageDependencyError::PackageStatusError(error)),
                        Ok(status) => {
                            let dependency = PackageDependency {
                                id: dependency_record.id().clone(),
                                version: version.clone(),
                                level,
                                status
                            };
                            push_if_not_exists(dependency, &mut result);
                        }
                    }
                },
                None => {
                    // the given package id does not exist
                    return Err(PackageDependencyError::PackageNotFound);
                }
            }
        }

        return Ok(result);
    }

    // pub fn find_virtual_dependencies(&self, record: &PackageRecord) -> HashMap<AbsoluteVirtualKey, PackageStatus> {
    //     unimplemented!()
    // }

    pub fn download<F>(&self, record: &PackageRecord, progress: F) -> Result<PathBuf, crate::download::DownloadError>
            where F: Fn(u64, u64) -> () + Send + 'static {
        let installer = match record.package().installer() {
            None => return Err(crate::download::DownloadError::NoUrl),
            Some(v) => v
        };

        let mut disposable = record.package().download(&self.download_path(&installer.url()), Some(progress)).unwrap();
        let v = disposable.wait();
        Ok(v.unwrap())
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

    fn download_path(&self, url: &str) -> PathBuf {
        let mut sha = Sha256::new();
        sha.input_str(url);
        let hash_id = sha.result_str();
        
        self.config.read().unwrap().package_cache_path().join(hash_id)
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
