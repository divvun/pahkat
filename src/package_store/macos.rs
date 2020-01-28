use std::collections::BTreeMap;
use std::fmt::Display;
use std::fs::{remove_dir, remove_file};
use std::io;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::str::FromStr;
use std::sync::{Arc, RwLock};

use hashbrown::HashMap;
use pahkat_types::{Downloadable, InstallTarget, Installer, MacOSInstaller, Package};
use serde::de::{self, Deserializer};
use serde::Deserialize;
use snafu::ResultExt;
use url::Url;

use super::{PackageStore, SharedRepos, SharedStoreConfig};
use crate::download::DownloadManager;
use crate::{cmp, PackageKey, RepoRecord, StoreConfig};

#[cfg(target_os = "macos")]
pub fn global_uninstall_path() -> PathBuf {
    PathBuf::from("/Library/Application Support/Pahkat/uninstall")
}

use crate::transaction::{PackageStatus, PackageStatusError};

use crate::transaction::{install::InstallError, install::ProcessError, uninstall::UninstallError};

fn from_str<'de, T, D>(deserializer: D) -> Result<T, D::Error>
where
    T: FromStr,
    T::Err: Display,
    D: Deserializer<'de>,
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

pub struct MacOSPackageStore {
    repos: SharedRepos,
    config: SharedStoreConfig,
}

impl PackageStore for MacOSPackageStore {
    type Target = InstallTarget;

    fn repos(&self) -> SharedRepos {
        Arc::clone(&self.repos)
    }

    fn config(&self) -> SharedStoreConfig {
        Arc::clone(&self.config)
    }

    fn install(
        &self,
        key: &PackageKey,
        target: &InstallTarget,
    ) -> Result<PackageStatus, InstallError> {
        let package = match self.find_package_by_key(key) {
            Some(v) => v,
            None => {
                return Err(InstallError::NoPackage);
            }
        };

        let installer = match package.installer() {
            None => return Err(InstallError::NoInstaller),
            Some(v) => v,
        };

        let installer = match installer {
            Installer::MacOS(ref v) => v,
            _ => return Err(InstallError::WrongInstallerType),
        };

        let url = url::Url::parse(&installer.url).map_err(|source| InstallError::InvalidUrl {
            source,
            url: installer.url.to_owned(),
        })?;
        let filename = url.path_segments().unwrap().last().unwrap();
        let pkg_path =
            crate::repo::download_path(&self.config.read().unwrap(), &url.as_str()).join(filename);

        log::debug!("Installing {}: {:?}", &key, &pkg_path);

        if !pkg_path.exists() {
            log::error!("Package path doesn't exist: {:?}", &pkg_path);
            return Err(InstallError::PackageNotInCache);
        }

        install_macos_package(&pkg_path, &target)
            .context(crate::transaction::install::InstallerFailure {})?;

        Ok(self.status_impl(&installer, key, &package, target).unwrap())
    }

    fn uninstall(
        &self,
        key: &PackageKey,
        target: &InstallTarget,
    ) -> Result<PackageStatus, UninstallError> {
        let package = match self.find_package_by_key(key) {
            Some(v) => v,
            None => {
                return Err(UninstallError::NoPackage);
            }
        };

        let installer = match package.installer() {
            None => return Err(UninstallError::NoInstaller),
            Some(v) => v,
        };

        let installer = match installer {
            &Installer::MacOS(ref v) => v,
            _ => return Err(UninstallError::WrongInstallerType),
        };

        match uninstall_macos_package(&installer.pkg_id, &target) {
            Err(e) => return Err(UninstallError::ProcessFailed { source: e }),
            _ => {}
        };

        Ok(self.status_impl(installer, key, &package, target).unwrap())
    }

    fn import(
        &self,
        key: &PackageKey,
        installer_path: &Path,
    ) -> Result<PathBuf, Box<dyn std::error::Error>> {
        let package = match self.find_package_by_key(key) {
            Some(v) => v,
            None => {
                return Err(Box::new(crate::download::DownloadError::NoUrl) as _);
            }
        };

        let installer = match package.installer() {
            None => return Err(Box::new(crate::download::DownloadError::NoUrl) as _),
            Some(v) => v,
        };

        let config = &self.config.read().unwrap();

        let output_path = crate::repo::download_path(config, &installer.url());
        std::fs::copy(installer_path, &output_path)?;
        Ok(output_path)
    }

    fn download(
        &self,
        key: &PackageKey,
        progress: Box<dyn Fn(u64, u64) -> bool + Send + 'static>,
    ) -> Result<PathBuf, crate::download::DownloadError> {
        let package = match self.find_package_by_key(key) {
            Some(v) => v,
            None => {
                return Err(crate::download::DownloadError::NoUrl);
            }
        };

        let installer = match package.installer() {
            None => return Err(crate::download::DownloadError::NoUrl),
            Some(v) => v,
        };

        let url = match Url::parse(&*installer.url()) {
            Ok(v) => v,
            Err(e) => return Err(crate::download::DownloadError::InvalidUrl),
        };

        let config = &self.config.read().unwrap();
        let dm = DownloadManager::new(
            config.download_cache_path(),
            config.max_concurrent_downloads(),
        );

        let output_path = crate::repo::download_path(config, &installer.url());
        crate::block_on(dm.download(&url, output_path, Some(progress)))
    }

    fn status(
        &self,
        key: &PackageKey,
        target: &Self::Target,
    ) -> Result<PackageStatus, PackageStatusError> {
        let package = match self.find_package_by_key(key) {
            Some(v) => v,
            None => {
                return Err(PackageStatusError::NoPackage);
            }
        };

        let installer = match package.installer() {
            None => return Err(PackageStatusError::NoInstaller),
            Some(v) => v,
        };

        let installer = match installer {
            Installer::MacOS(ref v) => v,
            _ => return Err(PackageStatusError::WrongInstallerType),
        };

        self.status_impl(installer, key, &package, target)
    }

    fn all_statuses(
        &self,
        repo_record: &RepoRecord,
        target: &InstallTarget,
    ) -> BTreeMap<String, Result<PackageStatus, PackageStatusError>> {
        crate::repo::all_statuses(self, repo_record, target)
    }

    fn find_package_by_key(&self, key: &PackageKey) -> Option<Package> {
        crate::repo::find_package_by_key(key, &self.repos)
    }

    fn find_package_by_id(&self, package_id: &str) -> Option<(PackageKey, Package)> {
        crate::repo::find_package_by_id(self, package_id, &self.repos)
    }

    fn refresh_repos(&self) {
        let config = self.config.read().unwrap();
        *self.repos.write().unwrap() = crate::repo::refresh_repos(&*config);
    }

    fn clear_cache(&self) {
        crate::repo::clear_cache(&self.config.read().unwrap())
    }

    fn add_repo(&self, url: String, channel: String) -> Result<bool, Box<dyn std::error::Error>> {
        &self.config.read().unwrap().add_repo(RepoRecord {
            url: Url::parse(&url).unwrap(),
            channel,
        })?;
        self.refresh_repos();
        Ok(true)
    }

    fn remove_repo(
        &self,
        url: String,
        channel: String,
    ) -> Result<bool, Box<dyn std::error::Error>> {
        self.config.read().unwrap().remove_repo(RepoRecord {
            url: Url::parse(&url).unwrap(),
            channel,
        })?;
        self.refresh_repos();
        Ok(true)
    }

    fn update_repo(
        &self,
        index: usize,
        url: String,
        channel: String,
    ) -> Result<bool, Box<dyn std::error::Error>> {
        self.config.read().unwrap().update_repo(
            index,
            RepoRecord {
                url: Url::parse(&url).unwrap(),
                channel,
            },
        )?;
        self.refresh_repos();
        Ok(true)
    }
}

impl std::default::Default for MacOSPackageStore {
    fn default() -> Self {
        let config = StoreConfig::load_or_default(true);
        MacOSPackageStore::new(config)
    }
}

impl MacOSPackageStore {
    pub fn new(config: StoreConfig) -> MacOSPackageStore {
        let store = MacOSPackageStore {
            repos: Arc::new(RwLock::new(HashMap::new())),
            config: Arc::new(RwLock::new(config)),
        };

        store.refresh_repos();

        store
    }

    fn status_impl(
        &self,
        installer: &MacOSInstaller,
        id: &PackageKey,
        package: &Package,
        target: &InstallTarget,
    ) -> Result<PackageStatus, PackageStatusError> {
        let pkg_info = match get_package_info(&installer.pkg_id, target) {
            Ok(v) => v,
            Err(e) => {
                match e {
                    ProcessError::NotFound => {}
                    _ => {
                        log::error!("{:?}", e);
                    }
                };

                return Ok(PackageStatus::NotInstalled);
            }
        };

        let config = self.config.read().unwrap();
        let skipped_package = config.skipped_package(id);
        let skipped_package = skipped_package.as_ref().map(String::as_ref);

        let status = self::cmp::cmp(&pkg_info.pkg_version, &package.version, skipped_package);

        status
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
    pub uid: u64,
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
    pub volume: String,
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

fn get_package_info(
    bundle_id: &str,
    target: &InstallTarget,
) -> Result<MacOSPackageExportPlist, ProcessError> {
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
            log::error!("pkgutil: {:?}", &e);
            return Err(ProcessError::Io { source: e });
        }
    };

    if !output.status.success() {
        if let Some(code) = output.status.code() {
            if code == 1 {
                return Err(ProcessError::NotFound);
            }
        }

        log::error!("pkgutil: {:?}", &output);
        return Err(ProcessError::Unknown { output });
    }

    let plist_data = String::from_utf8(output.stdout).expect("plist should always be valid UTF-8");
    let cursor = Cursor::new(plist_data);
    let plist: MacOSPackageExportPlist =
        plist::from_reader(cursor).expect("plist should always be valid");
    return Ok(plist);
}

fn install_macos_package(pkg_path: &Path, target: &InstallTarget) -> Result<(), ProcessError> {
    let target_str = match target {
        InstallTarget::User => "CurrentUserHomeDirectory",
        InstallTarget::System => "LocalSystem",
    };

    let args = &["-pkg", &pkg_path.to_str().unwrap(), "-target", target_str];
    log::debug!("Running command: 'installer {}'", args.join(" "));

    let res = Command::new("installer").args(args).output();
    let output = match res {
        Ok(v) => v,
        Err(e) => {
            log::error!("{:?}", &e);
            return Err(ProcessError::Io { source: e });
        }
    };
    if !output.status.success() {
        log::error!("{:?}", &output);
        let _msg = format!("Exit code: {}", output.status.code().unwrap());
        return Err(ProcessError::Unknown { output });
    }
    Ok(())
}

fn run_script(name: &str, bundle_id: &str, target: &InstallTarget) -> Result<(), ProcessError> {
    let path = match target {
        InstallTarget::User => crate::defaults::uninstall_path(),
        InstallTarget::System => global_uninstall_path(),
    };
    let script_path = path.join(bundle_id).join(name);

    if !is_executable::is_executable(&script_path) {
        return Ok(());
    }

    let res = Command::new(&script_path).output();
    let output = match res {
        Ok(v) => v,
        Err(e) => {
            log::error!("{:?}", &e);
            return Err(ProcessError::Io { source: e });
        }
    };
    if !output.status.success() {
        log::error!("{:?}", &output);
        let _msg = format!("Exit code: {}", output.status.code().unwrap());
        return Err(ProcessError::Unknown { output });
    }
    Ok(())
}

fn run_pre_uninstall_script(bundle_id: &str, target: &InstallTarget) -> Result<(), ProcessError> {
    run_script("pre-uninstall", bundle_id, target)
}

fn run_post_uninstall_script(bundle_id: &str, target: &InstallTarget) -> Result<(), ProcessError> {
    run_script("post-uninstall", bundle_id, target)
}

fn uninstall_macos_package(bundle_id: &str, target: &InstallTarget) -> Result<(), ProcessError> {
    let package_info = get_package_info(bundle_id, target)?;

    run_pre_uninstall_script(bundle_id, target)?;

    let mut errors = vec![];
    let mut directories = vec![];

    for path in package_info.paths() {
        let meta = match path.symlink_metadata() {
            Ok(v) => v,
            Err(err) => {
                errors.push(err);
                continue;
            }
        };

        if meta.is_dir() {
            directories.push(path);
            continue;
        }

        log::error!("Deleting: {:?}", &path);
        match remove_file(&path) {
            Err(err) => match err.kind() {
                io::ErrorKind::NotFound => {}
                _ => {
                    log::error!("{:?}: {:?}", &path, &err);
                    errors.push(err);
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
        log::error!("Deleting: {:?}", &dir);
        match remove_dir(&dir) {
            Err(err) => {
                log::error!("{:?}: {:?}", &dir, &err);
                errors.push(err);
            }
            Ok(_) => {}
        }
    }

    log::error!("{:?}", errors);

    forget_pkg_id(bundle_id, target)?;

    run_post_uninstall_script(bundle_id, target)?;

    Ok(())
}

fn forget_pkg_id(bundle_id: &str, target: &InstallTarget) -> Result<(), ProcessError> {
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
            log::error!("{:?}", e);
            return Err(ProcessError::Io { source: e });
        }
    };
    if !output.status.success() {
        log::error!("{:?}", output.status.code().unwrap());
        return Err(ProcessError::Unknown { output });
    }
    Ok(())
}
