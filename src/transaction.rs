use crate::{AbsolutePackageKey, PackageStatus, PackageStatusError};
use crypto::digest::Digest;
use crypto::sha2::Sha256;
use hashbrown::HashMap;
use pahkat_types::{Downloadable, InstallTarget, Installer, MacOSInstaller, Package};
use serde::de::{self, Deserialize, Deserializer};
use snafu::Snafu;
use std::collections::BTreeMap;
use std::fmt;
use std::fmt::Display;
use std::fs::{remove_dir, remove_file};
use std::io;
use std::path::{Path, PathBuf};
use std::process::{self, Command};
use std::str::FromStr;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc, RwLock,
};
use url::Url;

pub mod install;
pub mod uninstall;

use install::InstallError;
use uninstall::UninstallError;

pub trait PackageStore: Send + Sync {
    type Target: Send + Sync;

    fn download(
        &self,
        key: &AbsolutePackageKey,
        progress: Box<dyn Fn(u64, u64) -> () + Send + 'static>,
    ) -> Result<PathBuf, crate::download::DownloadError>;

    fn resolve_package(&self, key: &AbsolutePackageKey) -> Option<Package>;

    fn install(
        &self,
        key: &AbsolutePackageKey,
        target: &Self::Target,
    ) -> Result<PackageStatus, InstallError>;

    fn uninstall(
        &self,
        key: &AbsolutePackageKey,
        target: &Self::Target,
    ) -> Result<PackageStatus, UninstallError>;

    fn status(
        &self,
        key: &AbsolutePackageKey,
        target: &InstallTarget,
    ) -> Result<PackageStatus, PackageStatusError>;

    fn find_package_by_id(&self, package_id: &str) -> Option<(AbsolutePackageKey, Package)>;

    fn find_package_dependencies(
        &self,
        key: &AbsolutePackageKey,
        package: &Package,
        target: &Self::Target,
    ) -> Result<Vec<crate::PackageDependency>, PackageDependencyError>;

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
}

pub trait PackageTarget: Send + Sync + Clone {}

/// This is so good.
impl PackageTarget for () {}

#[derive(Debug, Clone)]
pub struct PackageAction<T: PackageTarget> {
    pub id: AbsolutePackageKey,
    pub action: PackageActionType,
    pub target: T,
}

impl<T: PackageTarget> PackageAction<T> {
    pub fn install(id: AbsolutePackageKey, target: T) -> PackageAction<T> {
        PackageAction {
            id,
            action: PackageActionType::Install,
            target,
        }
    }

    pub fn uninstall(id: AbsolutePackageKey, target: T) -> PackageAction<T> {
        PackageAction {
            id,
            action: PackageActionType::Uninstall,
            target,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub enum PackageDependencyError {
    PackageNotFound,
    VersionNotFound,
    PackageStatusError(PackageStatusError),
}

impl fmt::Display for PackageDependencyError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match *self {
            PackageDependencyError::PackageNotFound => write!(f, "Error: Package not found"),
            PackageDependencyError::VersionNotFound => {
                write!(f, "Error: Package version not found")
            }
            PackageDependencyError::PackageStatusError(e) => write!(f, "{}", e),
        }
    }
}
pub struct PackageTransaction<T: PackageTarget> {
    store: Arc<dyn PackageStore<Target = T>>,
    actions: Arc<Vec<PackageAction<T>>>,
    is_cancelled: Arc<AtomicBool>,
}

// pub struct PackageTransaction {
//     store: Arc<MacOSPackageStore>,
//     actions: Arc<Vec<PackageAction>>,
//     is_cancelled: Arc<AtomicBool>,
// }

#[derive(Debug)]
pub enum TransactionEvent {
    NotStarted,
    Uninstalling,
    Installing,
    Completed,
    Error,
}

impl TransactionEvent {
    pub fn to_u32(&self) -> u32 {
        match self {
            TransactionEvent::NotStarted => 0,
            TransactionEvent::Uninstalling => 1,
            TransactionEvent::Installing => 2,
            TransactionEvent::Completed => 3,
            TransactionEvent::Error => 4,
        }
    }
}

#[derive(Debug, Clone)]
pub enum PackageTransactionError {
    NoPackage(String),
    Deps(PackageDependencyError),
    ActionContradiction(String),
}

impl fmt::Display for PackageTransactionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "{:?}", self)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum PackageActionType {
    Install,
    Uninstall,
}

impl PackageActionType {
    pub fn from_u8(x: u8) -> PackageActionType {
        match x {
            0 => PackageActionType::Install,
            1 => PackageActionType::Uninstall,
            _ => panic!("Invalid package action type: {}", x),
        }
    }

    pub fn to_u8(&self) -> u8 {
        match self {
            PackageActionType::Install => 0,
            PackageActionType::Uninstall => 1,
        }
    }
}

impl<T: PackageTarget + 'static> PackageTransaction<T> {
    pub fn new(
        store: Arc<dyn PackageStore<Target = T>>,
        actions: Vec<PackageAction<T>>,
    ) -> Result<PackageTransaction<T>, PackageTransactionError> {
        let mut new_actions: Vec<PackageAction<T>> = vec![];

        for action in actions.iter() {
            let package_key = &action.id;

            let package = match store.resolve_package(&package_key) {
                Some(p) => p,
                None => {
                    return Err(PackageTransactionError::NoPackage(package_key.to_string()));
                }
            };

            if action.action == PackageActionType::Install {
                let dependencies =
                    match store.find_package_dependencies(&action.id, &package, &action.target) {
                        Ok(d) => d,
                        Err(e) => return Err(PackageTransactionError::Deps(e)),
                    };

                for dependency in dependencies.into_iter() {
                    let contradiction = actions.iter().find(|action| {
                        dependency.id == action.id && action.action == PackageActionType::Uninstall
                    });
                    match contradiction {
                        Some(_) => {
                            return Err(PackageTransactionError::ActionContradiction(
                                package_key.to_string(),
                            ))
                        }
                        None => {
                            if !new_actions.iter().any(|x| x.id == dependency.id) {
                                let new_action = PackageAction {
                                    id: dependency.id,
                                    action: PackageActionType::Install,
                                    target: action.target.clone(),
                                };
                                new_actions.push(new_action);
                            }
                        }
                    }
                }
            }
            if !new_actions.iter().any(|x| x.id == action.id) {
                new_actions.push(action.to_owned());
            }
        }

        Ok(PackageTransaction {
            store,
            actions: Arc::new(new_actions),
            is_cancelled: Arc::new(AtomicBool::new(false)),
        })
    }

    pub fn actions(&self) -> Arc<Vec<PackageAction<T>>> {
        Arc::clone(&self.actions)
    }

    pub fn validate(&self) -> bool {
        true
    }

    pub fn process<F>(&mut self, progress: F)
    where
        F: Fn(AbsolutePackageKey, TransactionEvent) -> () + 'static + Send,
    {
        if !self.validate() {
            // TODO: early return
            return;
        }

        let is_cancelled = self.is_cancelled.clone();
        let store = Arc::new(self.store.clone());
        let actions = self.actions.clone();

        let handle = std::thread::spawn(move || {
            for action in actions.iter() {
                if is_cancelled.load(Ordering::Relaxed) == true {
                    return;
                }

                match action.action {
                    PackageActionType::Install => {
                        progress(action.id.clone(), TransactionEvent::Installing);
                        match store.install(&action.id, &action.target) {
                            Ok(_) => progress(action.id.clone(), TransactionEvent::Completed),
                            Err(e) => {
                                eprintln!("{:?}", &e);
                                progress(action.id.clone(), TransactionEvent::Error)
                            }
                        };
                    }
                    PackageActionType::Uninstall => {
                        progress(action.id.clone(), TransactionEvent::Uninstalling);
                        match store.uninstall(&action.id, &action.target) {
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

        handle.join().expect("handle failed to join");
    }

    pub fn cancel(&self) -> bool {
        // let prev_value = *self.is_cancelled.read().unwrap();
        // *self.is_cancelled.write().unwrap() = true;
        // prev_value
        unimplemented!()
    }
}
