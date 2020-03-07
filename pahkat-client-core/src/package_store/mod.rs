#[cfg(all(target_os = "macos", feature = "macos"))]
pub mod macos;

#[cfg(feature = "prefix")]
pub mod prefix;

#[cfg(all(windows, feature = "windows"))]
pub mod windows;

use crate::transaction::{PackageStatus, PackageStatusError};
use crate::{LoadedRepository, PackageKey};
use hashbrown::HashMap;
use std::collections::BTreeMap;
use std::sync::{Arc, RwLock};
use url::Url;

use crate::config::Config;
use crate::transaction::{install::InstallError, uninstall::UninstallError};
use pahkat_types::package::Package;
use std::path::{Path, PathBuf};

pub type SharedStoreConfig = Arc<RwLock<Config>>;
pub type SharedRepos = Arc<RwLock<HashMap<Url, LoadedRepository>>>;

#[derive(Debug, thiserror::Error)]
pub enum ImportError {
    #[error("Payload error")]
    Payload(#[from] crate::repo::PayloadError),

    #[error("IO error")]
    Io(#[from] std::io::Error),

    #[error("Invalid payload type")]
    InvalidPayloadType,
}

pub trait PackageStore: Send + Sync {
    type Target: Send + Sync;

    fn repos(&self) -> SharedRepos;
    fn config(&self) -> SharedStoreConfig;

    fn download(
        &self,
        key: &PackageKey,
        progress: Box<dyn Fn(u64, u64) -> bool + Send + 'static>,
    ) -> Result<PathBuf, crate::download::DownloadError>;

    fn import(&self, key: &PackageKey, installer_path: &Path) -> Result<PathBuf, ImportError>;

    fn install(
        &self,
        key: &PackageKey,
        target: &Self::Target,
    ) -> Result<PackageStatus, InstallError>;

    fn uninstall(
        &self,
        key: &PackageKey,
        target: &Self::Target,
    ) -> Result<PackageStatus, UninstallError>;

    fn status(
        &self,
        key: &PackageKey,
        target: &Self::Target,
    ) -> Result<PackageStatus, PackageStatusError>;

    fn all_statuses(
        &self,
        repo_url: &Url,
        target: &Self::Target,
    ) -> BTreeMap<String, Result<PackageStatus, PackageStatusError>>;

    fn find_package_by_id(&self, package_id: &str) -> Option<(PackageKey, Package)>;

    fn find_package_by_key(&self, key: &PackageKey) -> Option<Package>;

    fn refresh_repos(&self);

    fn clear_cache(&self);

    fn force_refresh_repos(&self) {
        self.clear_cache();
        self.refresh_repos();
    }
}
