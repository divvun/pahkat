
use std::path::{Path, PathBuf};
use std::fs::{remove_file, remove_dir};
use std::fmt::Display;
use std::str::FromStr;
use std::process::{self, Command};
use hashbrown::HashMap;
use std::collections::BTreeMap;
use std::sync::{Arc, RwLock, atomic::{AtomicBool, Ordering}};
use std::io;

use url::Url;
use serde::de::{self, Deserialize, Deserializer};
use crypto::digest::Digest;
use crypto::sha2::Sha256;
use pahkat::types::{
    InstallTarget,
    Installer,
    MacOSInstaller,
    Package,
    Downloadable
};

use crate::{
    PackageStatus,
    PackageStatusError,
    PackageDependency,
    PackageDependencyError,
    StoreConfig,
    AbsolutePackageKey,
    PackageActionType,
    PackageTransactionError,
    TransactionEvent,
    RepoRecord,
    repo::Repository,
    download::Download,
    cmp,
    default_uninstall_path,
    global_uninstall_path
};

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

#[derive(Debug)]
pub enum InstallError {
    NoPackage,
    NoInstaller,
    WrongInstallerType,
    InvalidFileType,
    PackageNotInCache,
    InvalidUrl(String),
    InstallerFailure(ProcessError)
}

#[derive(Debug)]
pub enum UninstallError {
    NoPackage,
    NoInstaller,
    WrongInstallerType,
    PkgutilFailure(ProcessError)
}

#[derive(Debug, Clone, Serialize)]
pub struct PackageAction {
    pub id: AbsolutePackageKey,
    pub action: PackageActionType,
    pub target: InstallTarget
}

pub struct TransactionDisposable {
    is_cancelled: Arc<AtomicBool>,
    // result: Option<Result<PathBuf, DownloadError>>,
    // handle: Option<JoinHandle<Result<PathBuf, DownloadError>>>
}

pub struct PackageTransaction {
    store: Arc<MacOSPackageStore>,
    actions: Arc<Vec<PackageAction>>,
    is_cancelled: Arc<AtomicBool>
}

impl PackageTransaction {
    pub fn new(
        store: Arc<MacOSPackageStore>,
        actions: Vec<PackageAction>
    ) -> Result<PackageTransaction, PackageTransactionError> {
        let mut new_actions: Vec<PackageAction> = vec![];

        for action in actions.iter() {
            let package_key = &action.id;

            let package = match store.resolve_package(&package_key) {
                Some(p) => p,
                None => {
                    return Err(PackageTransactionError::NoPackage(package_key.to_string()));
                }
            };

            if action.action == PackageActionType::Install {
                let dependencies = match store.find_package_dependencies(&action.id, &package, action.target) {
                    Ok(d) => d,
                    Err(e) => return Err(PackageTransactionError::Deps(e))
                };

                for dependency in dependencies.into_iter() {
                    let contradiction = actions.iter().find(|action| {
                        dependency.id == action.id && action.action == PackageActionType::Uninstall
                    });
                    match contradiction {
                        Some(a) => {
                            return Err(PackageTransactionError::ActionContradiction(package_key.to_string()))
                        },
                        None => {
                            if !new_actions.iter().any(|x| x.id == dependency.id) {
                                new_actions.push(PackageAction {
                                    id: dependency.id,
                                    action: PackageActionType::Install,
                                    target: action.target
                                })
                            }
                        }
                    }
                }
            }
            if !new_actions.iter().any(|x| x.id == action.id) {
                new_actions.push(action.clone());
            }
        }

        Ok(PackageTransaction {
            store,
            actions: Arc::new(new_actions),
            is_cancelled: Arc::new(AtomicBool::new(false))
        })
    }

    pub fn actions(&self) -> Arc<Vec<PackageAction>> {
        self.actions.clone()
    }

    pub fn validate(&self) -> bool {
        true
    }

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
                        progress(action.id.clone(), TransactionEvent::Installing);
                        match store.install(&action.id, action.target) {
                            Ok(_) => progress(action.id.clone(), TransactionEvent::Completed),
                            Err(e) => {
                                eprintln!("{:?}", &e);
                                progress(action.id.clone(), TransactionEvent::Error)
                            }
                        };
                    },
                    PackageActionType::Uninstall => {
                        progress(action.id.clone(), TransactionEvent::Uninstalling);
                        match store.uninstall(&action.id, action.target) {
                            Ok(_) => progress(action.id.clone(), TransactionEvent::Completed),
                            Err(e) => {
                                eprintln!("{:?}", &e);
                                progress(action.id.clone(), TransactionEvent::Error)
                            }
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

    fn clear_cache(&self) {
        let config = self.config.read().unwrap();
        for record in config.repos().iter() {
            match Repository::clear_cache(
                &record.url,
                record.channel.clone(),
                &config.repo_cache_path()
            ) {
                Err(e) => { println!("{:?}", e); }
                Ok(_) => {},
            };
        }
    }

    pub fn force_refresh_repos(&self) {
        self.clear_cache();
        self.refresh_repos();
    }

    fn recurse_linked_repos(&self, url: &str, channel: String, repos: &mut HashMap<RepoRecord, Repository>, cache_path: &Path) {
        let url = match url::Url::parse(url) {
            Ok(v) => v,
            Err(e) => { 
                eprintln!("{:?}", e);
                return;
            }
        };

        let record = RepoRecord {
            url,
            channel
        };

        self.recurse_repo(&record, repos, cache_path);
    }

    fn recurse_repo(&self, record: &RepoRecord, repos: &mut HashMap<RepoRecord, Repository>, cache_path: &Path) {
        if repos.contains_key(&record) {
            return;
        }

        match Repository::from_cache_or_url(
            &record.url,
            record.channel.clone(),
            cache_path
        ) {
            Ok(repo) => {
                for url in repo.meta().linked_repositories.iter() {
                    self.recurse_linked_repos(url, record.channel.clone(), repos, cache_path);
                }

                repos.insert(record.clone(), repo);
            },
            // TODO: actual error handling omg
            Err(e) => { eprintln!("{:?}", e); }
        };
    }
    
    pub fn refresh_repos(&self) {
        let mut repos = HashMap::new();
        let config = self.config.read().unwrap();

        for record in config.repos().iter() {
            self.recurse_repo(record, &mut repos, &config.repo_cache_path());
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

    pub fn resolve_package(&self, package_key: &AbsolutePackageKey) -> Option<Package> {
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
                    Some(x) => Some(x.to_owned()),
                    None => None
                };
                println!("Found pkg: {:?}", &pkg);
                pkg
            })
    }

    pub fn find_package(&self, package_id: &str) -> Option<(AbsolutePackageKey, Package)> {
        self.repos.read().unwrap().iter()
            .find_map(|(key, repo)| {
                repo.packages()
                    .get(package_id)
                    .map(|x| (AbsolutePackageKey::new(repo.meta(), &key.channel, package_id), x.to_owned()))
            })
    }

    /// Get the dependencies for a given package
    pub fn find_package_dependencies(&self, key: &AbsolutePackageKey, package: &Package, target: InstallTarget) -> Result<Vec<PackageDependency>, PackageDependencyError> {
        let mut resolved = Vec::<String>::new();
        Ok(self.find_package_dependencies_impl(key, package, target, 0, &mut resolved)?)
    }

    fn find_package_dependencies_impl(
        &self,
        key: &AbsolutePackageKey,
        package: &Package,
        target: InstallTarget,
        level: u8,
        resolved: &mut Vec<String>
    ) -> Result<Vec<PackageDependency>, PackageDependencyError> {
        fn push_if_not_exists(dependency: PackageDependency, result: &mut Vec<PackageDependency>) {
            if result.iter().filter(|d| d.id == dependency.id).count() == 0 {
                result.push(dependency);
            }
        }

        let mut result = Vec::<PackageDependency>::new();

        for (package_id, version) in package.dependencies.iter() {
            // avoid circular references by keeping
            // track of package ids that have already been processed
            if resolved.contains(package_id) {
                continue;
            }
            resolved.push(package_id.clone());

            match self.find_package(package_id.as_str()) {
                Some((ref key, ref package)) => {
                    // add all the dependencies of the dependency
                    // to the list result first
                    for dependency in self.find_package_dependencies_impl(key, package, target, level + 1, resolved)? {
                        push_if_not_exists(dependency, &mut result);
                    }

                    // make sure the version requirement is correct
                    // TODO: equality isn't how version comparisons work.
                    // if package.version.as_str() != version {
                    //     return Err(PackageDependencyError::VersionNotFound);
                    // }

                    match self.status(key, target) {
                        Err(error) => return Err(PackageDependencyError::PackageStatusError(error)),
                        Ok(status) => {
                            match status {
                                PackageStatus::UpToDate => {},
                                _ => {
                                    let dependency = PackageDependency {
                                        id: key.clone(),
                                        version: version.clone(),
                                        level,
                                        status
                                    };
                                    push_if_not_exists(dependency, &mut result);
                                }
                            }
                            
                        }
                    }
                },
                _ => {
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

    pub fn download<F>(&self, key: &AbsolutePackageKey, progress: F) -> Result<PathBuf, crate::download::DownloadError>
            where F: Fn(u64, u64) -> () + Send + 'static {
        let package = match self.resolve_package(key) {
            Some(v) => v,
            None => {
                return Err(crate::download::DownloadError::NoUrl);
            }
        };

        let installer = match package.installer() {
            None => return Err(crate::download::DownloadError::NoUrl),
            Some(v) => v
        };

        let mut disposable = package.download(&self.download_path(&installer.url()), Some(progress))?;
        disposable.wait()
    }

    pub fn install(&self, key: &AbsolutePackageKey, target: InstallTarget) -> Result<PackageStatus, InstallError> {
        let package = match self.resolve_package(key) {
            Some(v) => v,
            None => {
                return Err(InstallError::NoPackage);
            }
        };

        let installer = match package.installer() {
            None => return Err(InstallError::NoInstaller),
            Some(v) => v
        };

        let installer = match *installer {
            Installer::MacOS(ref v) => v,
            _ => return Err(InstallError::WrongInstallerType)
        };

        let url = url::Url::parse(&installer.url)
            .map_err(|_| InstallError::InvalidUrl(installer.url.to_owned()))?;
        let filename = url.path_segments().unwrap().last().unwrap();
        let pkg_path = self.download_path(&url.as_str()).join(filename);

        if !pkg_path.exists() {
            eprintln!("Package path doesn't exist: {:?}", &pkg_path);
            return Err(InstallError::PackageNotInCache)
        }
        
        match install_macos_package(&pkg_path, target) {
            Err(e) => return Err(InstallError::InstallerFailure(e)),
            _ => {}
        };

        Ok(self.status_impl(&installer, key, &package, target).unwrap())
    }

    pub fn uninstall(&self, key: &AbsolutePackageKey, target: InstallTarget) -> Result<PackageStatus, UninstallError> {
        let package = match self.resolve_package(key) {
            Some(v) => v,
            None => {
                return Err(UninstallError::NoPackage);
            }
        };

        let installer = match package.installer() {
            None => return Err(UninstallError::NoInstaller),
            Some(v) => v
        };

        let installer = match installer {
            &Installer::MacOS(ref v) => v,
            _ => return Err(UninstallError::WrongInstallerType)
        };

        match uninstall_macos_package(&installer.pkg_id, target) {
            Err(e) => return Err(UninstallError::PkgutilFailure(e)),
            _ => {}
        };

        Ok(self.status_impl(installer, key, &package, target).unwrap())
    }

    pub fn status(&self, key: &AbsolutePackageKey, target: InstallTarget) -> Result<PackageStatus, PackageStatusError> {
         let package = match self.resolve_package(key) {
            Some(v) => v,
            None => {
                return Err(PackageStatusError::NoPackage);
            }
        };

        let installer = match package.installer() {
            None => return Err(PackageStatusError::NoInstaller),
            Some(v) => v
        };

        let installer = match installer {
            &Installer::MacOS(ref v) => v,
            _ => return Err(PackageStatusError::WrongInstallerType)
        };

        self.status_impl(installer, key, &package, target)
    }

    fn download_path(&self, url: &str) -> PathBuf {
        let mut sha = Sha256::new();
        sha.input_str(url);
        let hash_id = sha.result_str();
        
        self.config.read().unwrap().package_cache_path().join(hash_id)
    }

    fn status_impl(&self, installer: &MacOSInstaller, id: &AbsolutePackageKey, package: &Package, target: InstallTarget) -> Result<PackageStatus, PackageStatusError> {
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

        let skipped_package = self.config().skipped_package(id);
        let skipped_package = skipped_package.as_ref().map(String::as_ref);

        self::cmp::semver_cmp(&pkg_info.pkg_version, &package.version, skipped_package)
            .or_else(|_| self::cmp::iso8601_cmp(&pkg_info.pkg_version, &package.version, skipped_package))
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

fn get_package_info(bundle_id: &str, target: InstallTarget) -> Result<MacOSPackageExportPlist, ProcessError> {
    use std::io::Cursor;

    let home_dir = dirs::home_dir().expect("Always find home directory");
    let mut args = vec!["--export-plist", bundle_id];
    if let InstallTarget::User = target {
        args.push("--volume");
        args.push(&home_dir.to_str().unwrap());
    }
    let res = Command::new("pkgutil").args(&args).output();
    let output = match res {
        Ok(v) => v,
        Err(e) => {
            eprintln!("{:?}", &e);
            return Err(ProcessError::Io(e));
        }
    };

    if !output.status.success() {
        if let Some(code) = output.status.code() {
            if code == 1 {
                eprintln!("pkgutil pkg not found");
                return Err(ProcessError::NotFound);
            }
        }
        
        eprintln!("{:?}", &output);
        return Err(ProcessError::Unknown(output));
    }

    let plist_data = String::from_utf8(output.stdout).expect("plist should always be valid UTF-8");
    let cursor = Cursor::new(plist_data);
    let plist: MacOSPackageExportPlist = plist::from_reader(cursor).expect("plist should always be valid");
    return Ok(plist);
}

#[derive(Debug)]
pub enum ProcessError {
    Io(io::Error),
    Unknown(process::Output),
    NotFound
}

fn install_macos_package(pkg_path: &Path, target: InstallTarget) -> Result<(), ProcessError> {
    let target_str = match target {
        InstallTarget::User => "CurrentUserHomeDirectory",
        InstallTarget::System => "LocalSystem"
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
        Err(e) => {
            eprintln!("{:?}", &e);
            return Err(ProcessError::Io(e))
        }
    };
    if !output.status.success() {
        eprintln!("{:?}", &output);
        let _msg = format!("Exit code: {}", output.status.code().unwrap());
        return Err(ProcessError::Unknown(output));
    }
    Ok(())
}

fn run_script(name: &str, bundle_id: &str, target: InstallTarget) -> Result<(), ProcessError> {
    let path = match target {
        InstallTarget::User => default_uninstall_path(),
        InstallTarget::System => global_uninstall_path()
    };
    let script_path = path.join(bundle_id).join(name);

    if !is_executable::is_executable(&script_path) {
        return Ok(());
    }

    let res = Command::new(&script_path).output();
    let output = match res {
        Ok(v) => v,
        Err(e) => {
            eprintln!("{:?}", &e);
            return Err(ProcessError::Io(e))
        }
    };
    if !output.status.success() {
        eprintln!("{:?}", &output);
        let _msg = format!("Exit code: {}", output.status.code().unwrap());
        return Err(ProcessError::Unknown(output));
    }
    Ok(())
}

fn run_pre_uninstall_script(bundle_id: &str, target: InstallTarget) -> Result<(), ProcessError> {
    run_script("pre-uninstall", bundle_id, target)
}

fn run_post_uninstall_script(bundle_id: &str, target: InstallTarget) -> Result<(), ProcessError> {
    run_script("post-uninstall", bundle_id, target)
}

fn uninstall_macos_package(bundle_id: &str, target: InstallTarget) -> Result<(), ProcessError> {
    let package_info = get_package_info(bundle_id, target)?;

    run_pre_uninstall_script(bundle_id, target)?;

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

    run_post_uninstall_script(bundle_id, target)?;

    Ok(())
}

fn forget_pkg_id(bundle_id: &str, target: InstallTarget) -> Result<(), ProcessError> {
    let home_dir = dirs::home_dir().expect("Always find home directory");
    let mut args = vec!["--forget", bundle_id];
    if let InstallTarget::User = target {
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
