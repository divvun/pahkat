#[cfg(all(target_os = "macos", feature = "macos"))]
pub mod macos;

#[cfg(feature = "prefix")]
pub mod prefix;

#[cfg(all(windows, feature = "windows"))]
pub mod windows;

use crate::transaction::{PackageStatus, PackageStatusError};
use crate::{PackageKey, RepoRecord, Repository};
use hashbrown::HashMap;
use std::collections::BTreeMap;
use std::sync::{Arc, RwLock};

use crate::transaction::{install::InstallError, uninstall::UninstallError};
use pahkat_types::Package;
use std::path::{Path, PathBuf};

pub type SharedStoreConfig = Arc<RwLock<crate::StoreConfig>>;
pub type SharedRepos = Arc<RwLock<HashMap<RepoRecord, Repository>>>;

pub trait PackageStore: Send + Sync {
    type Target: Send + Sync;

    fn repos(&self) -> SharedRepos;
    fn config(&self) -> SharedStoreConfig;

    fn download(
        &self,
        key: &PackageKey,
        progress: Box<dyn Fn(u64, u64) -> bool + Send + 'static>,
    ) -> Result<PathBuf, crate::download::DownloadError>;

    fn import(
        &self,
        key: &PackageKey,
        installer_path: &Path,
    ) -> Result<PathBuf, Box<dyn std::error::Error>>;

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

    fn find_package_by_id(&self, package_id: &str) -> Option<(PackageKey, Package)>;

    fn find_package_by_key(&self, key: &PackageKey) -> Option<Package>;

    fn refresh_repos(&self);

    fn clear_cache(&self);

    fn force_refresh_repos(&self) {
        self.clear_cache();
        self.refresh_repos();
    }

    fn add_repo(&self, url: String, channel: String) -> Result<bool, Box<dyn std::error::Error>>;

    fn remove_repo(&self, url: String, channel: String)
        -> Result<bool, Box<dyn std::error::Error>>;

    fn update_repo(
        &self,
        index: usize,
        url: String,
        channel: String,
    ) -> Result<bool, Box<dyn std::error::Error>>;

    fn all_statuses(
        &self,
        repo_record: &RepoRecord,
        target: &Self::Target,
    ) -> BTreeMap<String, Result<PackageStatus, PackageStatusError>>;
}
