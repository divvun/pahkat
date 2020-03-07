use std::collections::BTreeMap;
use std::fmt::Display;
use std::fs::{remove_dir, remove_file};
use std::io;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::str::FromStr;
use std::sync::{Arc, RwLock};

use hashbrown::HashMap;
use pahkat_types::package::Package;
use pahkat_types::payload::macos::{self, InstallTarget};
use serde::de::{self, Deserializer};
use serde::Deserialize;
use url::Url;

use super::{PackageStore, SharedRepos, SharedStoreConfig};
use crate::download::DownloadManager;
use crate::package_store::ImportError;
use crate::{cmp, Config, PackageKey};

#[cfg(target_os = "macos")]
#[inline(always)]
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
        install_target: &InstallTarget,
    ) -> Result<PackageStatus, InstallError> {
        let query = crate::repo::ReleaseQuery::from(key);
        let repos = self.repos.read().unwrap();

        let (target, release) =
            crate::repo::resolve_payload(key, query, &*repos).map_err(InstallError::Payload)?;
        let installer = match target.payload {
            pahkat_types::payload::Payload::MacOSPackage(v) => v,
            _ => return Err(InstallError::WrongPayloadType),
        };
        let pkg_path =
            crate::repo::download_file_path(&*self.config.read().unwrap(), &installer.url);
        log::debug!("Installing {}: {:?}", &key, &pkg_path);

        if !pkg_path.exists() {
            log::error!("Package path doesn't exist: {:?}", &pkg_path);
            return Err(InstallError::PackageNotInCache);
        }

        install_macos_package(&pkg_path, &install_target)
            .map_err(InstallError::InstallerFailure)?;

        Ok(self
            .status_impl(&release, &installer, key, install_target)
            .unwrap())
    }

    fn uninstall(
        &self,
        key: &PackageKey,
        install_target: &InstallTarget,
    ) -> Result<PackageStatus, UninstallError> {
        let query = crate::repo::ReleaseQuery::from(key);
        let repos = self.repos.read().unwrap();

        let (target, release) =
            crate::repo::resolve_payload(key, query, &*repos).map_err(UninstallError::Payload)?;
        let installer = match target.payload {
            pahkat_types::payload::Payload::MacOSPackage(v) => v,
            _ => return Err(UninstallError::WrongPayloadType),
        };

        uninstall_macos_package(&installer.pkg_id, &install_target)
            .map_err(UninstallError::UninstallerFailure)?;

        Ok(self
            .status_impl(&release, &installer, key, install_target)
            .unwrap())
    }

    fn import(&self, key: &PackageKey, installer_path: &Path) -> Result<PathBuf, ImportError> {
        use std::convert::TryInto;

        let query = crate::repo::ReleaseQuery::from(key);
        let repos = self.repos.read().unwrap();

        let (target, release) = crate::repo::resolve_payload(key, query, &*repos)?;
        let installer: macos::Package = target
            .payload
            .try_into()
            .map_err(|_| ImportError::InvalidPayloadType)?;
        let config = &self.config.read().unwrap();

        let output_path = crate::repo::download_dir(&**config, &installer.url);
        std::fs::copy(installer_path, &output_path)?;
        Ok(output_path)
    }

    fn download(
        &self,
        key: &PackageKey,
        progress: Box<dyn Fn(u64, u64) -> bool + Send + 'static>,
    ) -> Result<PathBuf, crate::download::DownloadError> {
        let query = crate::repo::ReleaseQuery::from(key);
        let repos = self.repos.read().unwrap();
        crate::repo::download(&self.config, key, query, &*repos, progress)
    }

    fn status(
        &self,
        key: &PackageKey,
        install_target: &Self::Target,
    ) -> Result<PackageStatus, PackageStatusError> {
        let query = crate::repo::ReleaseQuery::from(key);
        let repos = self.repos.read().unwrap();

        let (target, release) = crate::repo::resolve_payload(key, query, &*repos)
            .map_err(PackageStatusError::Payload)?;
        let installer = match target.payload {
            pahkat_types::payload::Payload::MacOSPackage(v) => v,
            _ => return Err(PackageStatusError::WrongPayloadType),
        };

        self.status_impl(&release, &installer, key, install_target)
    }

    fn all_statuses(
        &self,
        repo_url: &Url,
        target: &InstallTarget,
    ) -> BTreeMap<String, Result<PackageStatus, PackageStatusError>> {
        crate::repo::all_statuses(self, repo_url, target)
    }

    fn find_package_by_key(&self, key: &PackageKey) -> Option<Package> {
        let repos = self.repos.read().unwrap();
        crate::repo::find_package_by_key(key, &*repos)
    }

    fn find_package_by_id(&self, package_id: &str) -> Option<(PackageKey, Package)> {
        let repos = self.repos.read().unwrap();
        crate::repo::find_package_by_id(self, package_id, &*repos)
    }

    fn refresh_repos(&self) {
        *self.repos.write().unwrap() = crate::repo::refresh_repos(&self.config);
    }

    fn clear_cache(&self) {
        crate::repo::clear_cache(&self.config)
    }
}

impl std::default::Default for MacOSPackageStore {
    fn default() -> Self {
        // let config = StoreConfig::load_or_default(true);
        // MacOSPackageStore::new(config)
        todo!()
    }
}

impl MacOSPackageStore {
    pub fn new(config: Arc<RwLock<Config>>) -> MacOSPackageStore {
        let store = MacOSPackageStore {
            repos: Arc::new(RwLock::new(HashMap::new())),
            config,
        };

        store.refresh_repos();

        store
    }

    fn status_impl(
        &self,
        release: &pahkat_types::package::Release,
        installer: &macos::Package,
        id: &PackageKey,
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
        let status = self::cmp::cmp(&pkg_info.pkg_version, &release.version);

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
            return Err(ProcessError::Io(e));
        }
    };

    if !output.status.success() {
        if let Some(code) = output.status.code() {
            if code == 1 {
                return Err(ProcessError::NotFound);
            }
        }

        log::error!("pkgutil: {:?}", &output);
        return Err(ProcessError::Unknown(output));
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
            return Err(ProcessError::Io(e));
        }
    };
    if !output.status.success() {
        log::error!("{:?}", &output);
        let _msg = format!("Exit code: {}", output.status.code().unwrap());
        return Err(ProcessError::Unknown(output));
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
            return Err(ProcessError::Io(e));
        }
    };
    if !output.status.success() {
        log::error!("{:?}", &output);
        let _msg = format!("Exit code: {}", output.status.code().unwrap());
        return Err(ProcessError::Unknown(output));
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
            return Err(ProcessError::Io(e));
        }
    };
    if !output.status.success() {
        log::error!("{:?}", output.status.code().unwrap());
        return Err(ProcessError::Unknown(output));
    }
    Ok(())
}
