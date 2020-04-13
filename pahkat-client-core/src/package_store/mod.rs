#[cfg(all(target_os = "macos", feature = "macos"))]
pub mod macos;
#[cfg(feature = "prefix")]
pub mod prefix;
#[cfg(all(windows, feature = "windows"))]
pub mod windows;

use std::collections::BTreeMap;
use std::sync::{Arc, RwLock};
use std::path::{Path, PathBuf};
use std::pin::Pin;

use hashbrown::HashMap;
use pahkat_types::package::Package;
use serde::{Deserialize, Serialize};
use url::Url;

use crate::config::Config;
use crate::transaction::{install::InstallError, uninstall::UninstallError};
use crate::transaction::{PackageStatus, PackageStatusError};
use crate::{LoadedRepository, PackageKey};
use crate::repo::RepoDownloadError;

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

pub enum ProgressEvent<P, C, E> {
    Progress(P),
    Complete(C),
    Error(E),
}

pub type DownloadEvent = ProgressEvent<(u64, u64), PathBuf, crate::download::DownloadError>;

#[non_exhaustive]
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum InstallTarget {
    System,
    User,
}

impl Default for InstallTarget {
    fn default() -> Self {
        InstallTarget::System
    }
}

pub type Stream<T> = Pin<Box<dyn futures::stream::Stream<Item = T> + Send + Sync + 'static>>;
pub type Future<T> = Pin<Box<dyn std::future::Future<Output = T>>>;

pub trait PackageStore: Send + Sync {
    fn repos(&self) -> SharedRepos;
    fn config(&self) -> SharedStoreConfig;

    #[must_use]
    fn download(
        &self,
        key: &PackageKey,
    ) -> Stream<DownloadEvent>;

    fn import(&self, key: &PackageKey, installer_path: &Path) -> Result<PathBuf, ImportError>;

    fn install(
        &self,
        key: &PackageKey,
        target: InstallTarget,
    ) -> Result<PackageStatus, InstallError>;

    fn uninstall(
        &self,
        key: &PackageKey,
        target: InstallTarget,
    ) -> Result<PackageStatus, UninstallError>;

    fn status(
        &self,
        key: &PackageKey,
        target: InstallTarget,
    ) -> Result<PackageStatus, PackageStatusError>;

    fn all_statuses(
        &self,
        repo_url: &Url,
        target: InstallTarget,
    ) -> BTreeMap<String, Result<PackageStatus, PackageStatusError>>;

    fn find_package_by_id(&self, package_id: &str) -> Option<(PackageKey, Package)>;

    fn find_package_by_key(&self, key: &PackageKey) -> Option<Package>;

    #[must_use]
    fn refresh_repos(&self) -> Future<Result<(), RepoDownloadError>>;

    fn clear_cache(&self);

    #[must_use]
    fn force_refresh_repos(&self) -> Future<Result<(), RepoDownloadError>> {
        self.clear_cache();
        self.refresh_repos()
    }
}
