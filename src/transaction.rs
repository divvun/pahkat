use crate::PackageKey;
use pahkat_types::{Package};
// use serde::de::{self, Deserialize, Deserializer};
use serde::{Deserialize, Serialize};
use std::fmt;
use std::path::PathBuf;

use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};

pub mod install;
pub mod uninstall;

use install::InstallError;
use uninstall::UninstallError;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "camelCase")]
pub enum PackageStatus {
    NotInstalled,
    UpToDate,
    RequiresUpdate,
    Skipped,
}

// impl PackageStatus {
//     fn to_u8(&self) -> u8 {
//         match self {
//             PackageStatus::NotInstalled => 0,
//             PackageStatus::UpToDate => 1,
//             PackageStatus::RequiresUpdate => 2,
//             PackageStatus::Skipped => 3
//         }
//     }
// }

impl fmt::Display for PackageStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(
            f,
            "{}",
            match *self {
                // PackageStatus::NoPackage => "No package",
                PackageStatus::NotInstalled => "Not installed",
                PackageStatus::UpToDate => "Up to date",
                PackageStatus::RequiresUpdate => "Requires update",
                PackageStatus::Skipped => "Skipped",
            }
        )
    }
}

impl fmt::Display for PackageStatusError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(
            f,
            "Error: {}",
            match *self {
                PackageStatusError::NoPackage => "No package",
                PackageStatusError::NoInstaller => "No installer",
                PackageStatusError::WrongInstallerType => "Wrong installer type",
                PackageStatusError::ParsingVersion => "Could not parse version",
                PackageStatusError::InvalidInstallPath => "Invalid install path",
                PackageStatusError::InvalidMetadata => "Invalid metadata",
            }
        )
    }
}

use std::sync::RwLock;
use hashbrown::HashMap;
use crate::RepoRecord;
use crate::repo::Repository;

pub trait PackageStore: Send + Sync {
    type Target: Send + Sync;

    fn repos(&self) -> Arc<RwLock<HashMap<RepoRecord, Repository>>>;

    fn download(
        &self,
        key: &PackageKey,
        progress: Box<dyn Fn(u64, u64) -> () + Send + 'static>,
    ) -> Result<PathBuf, crate::download::DownloadError>;

    fn resolve_package(&self, key: &PackageKey) -> Option<Package>;

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

    // fn find_package_dependencies(
    //     &self,
    //     key: &PackageKey,
    //     target: &Self::Target,
    // ) -> Result<Vec<???>, PackageDependencyError> {
    //     let package = match self.resolve_package(key) {
    //         Some(pkg) => pkg,
    //         None => return Err(PackageDependencyError::PackageNotFound(key.to_string()))
    //     };
        
    //     unimplemented!();
    // }

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
impl PackageTarget for pahkat_types::InstallTarget {}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PackageAction<T: PackageTarget> {
    pub id: PackageKey,
    pub action: PackageActionType,
    pub target: T,
}

impl<T: PackageTarget> PackageAction<T> {
    pub fn install(id: PackageKey, target: T) -> PackageAction<T> {
        PackageAction {
            id,
            action: PackageActionType::Install,
            target,
        }
    }

    pub fn uninstall(id: PackageKey, target: T) -> PackageAction<T> {
        PackageAction {
            id,
            action: PackageActionType::Uninstall,
            target,
        }
    }

    #[inline]
    pub fn is_install(&self) -> bool {
        self.action == PackageActionType::Install
    }

    #[inline]
    pub fn is_uninstall(&self) -> bool {
        self.action == PackageActionType::Uninstall
    }
}

#[derive(Debug, Clone, Copy)]
pub enum PackageStatusError {
    NoPackage,
    NoInstaller,
    WrongInstallerType,
    ParsingVersion,
    InvalidInstallPath,
    InvalidMetadata,
}

#[derive(Debug, Clone)]
pub enum PackageDependencyError {
    PackageNotFound(String),
    VersionNotFound(String),
    PackageStatusError(String, PackageStatusError),
}

impl fmt::Display for PackageDependencyError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            PackageDependencyError::PackageNotFound(x) => {
                write!(f, "Error: Package '{}' not found", x)
            }
            PackageDependencyError::VersionNotFound(x) => {
                write!(f, "Error: Package version '{}' not found", x)
            }
            PackageDependencyError::PackageStatusError(pkg, e) => write!(f, "{}: {}", pkg, e),
        }
    }
}

pub struct PackageTransaction<T: PackageTarget> {
    store: Arc<dyn PackageStore<Target = T>>,
    actions: Arc<Vec<PackageAction<T>>>,
    is_cancelled: Arc<AtomicBool>,
}

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
    InvalidStatus(crate::transaction::PackageStatusError)
}

impl std::error::Error for PackageTransactionError {}

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

fn process_install_action<T: PackageTarget + 'static>(
    store: &Arc<dyn PackageStore<Target = T>>,
    package: &Package,
    action: &PackageAction<T>,
    new_actions: &mut Vec<PackageAction<T>>,
) -> Result<(), PackageTransactionError> {
    let dependencies =
        match crate::repo::find_package_dependencies(store, &action.id, package, &action.target) {
            Ok(d) => d,
            Err(e) => return Err(PackageTransactionError::Deps(e)),
        };

    for dependency in dependencies.into_iter() {        
        if !new_actions.iter().any(|x| x.id == dependency.0) {
            // TODO: validate that it is allowed for user installations
            let new_action = PackageAction::install(dependency.0, action.target.clone());
            new_actions.push(new_action);
        }
    }

    Ok(())
}

use std::collections::HashSet;

impl<T: PackageTarget + 'static> PackageTransaction<T> {
    pub fn new(
        store: Arc<dyn PackageStore<Target = T>>,
        actions: Vec<PackageAction<T>>,
    ) -> Result<PackageTransaction<T>, PackageTransactionError> {
        let mut new_actions: Vec<PackageAction<T>> = vec![];

        // Collate all dependencies
        for action in actions.into_iter() {
            let package_key = &action.id;

            let package = store.resolve_package(&package_key).ok_or_else(||
                PackageTransactionError::NoPackage(package_key.to_string())
            )?;

            if action.is_install() {
                // Add all sub-dependencies
                process_install_action(&store, &package, &action, &mut new_actions)?;
            }
            
            if let Some(found_action) = new_actions.iter().find(|x| x.id == action.id) {
                if found_action.action != action.action {
                    return Err(PackageTransactionError::ActionContradiction(
                        action.id.to_string(),
                    ));
                }
            } else {
                new_actions.push(action);
            }
        }

        // Check for contradictions
        let mut installs = HashSet::new();
        let mut uninstalls = HashSet::new();

        for action in new_actions.iter() {
            if action.is_install() {
                installs.insert(&action.id);
            } else {
                uninstalls.insert(&action.id);
            }
        }

        // An intersection with more than 0 items is a contradiction.
        let contradictions = installs.intersection(&uninstalls).collect::<HashSet<_>>();
        if contradictions.len() > 0 {
            return Err(PackageTransactionError::ActionContradiction(
                format!("{:?}", contradictions),
            ));
        }

        // Check if packages need to even be installed or uninstalled
        let new_actions = new_actions.into_iter().try_fold(vec![], |mut out, action| {
            let status = store.status(&action.id, &action.target as _)
                .map_err(|err| {
                    PackageTransactionError::InvalidStatus(err)
                })?;
            
            let is_valid = if action.is_install() {
                status != PackageStatus::UpToDate
            } else {
                status == PackageStatus::UpToDate ||
                    status == PackageStatus::RequiresUpdate
            };

            if is_valid {
                out.push(action);
            }

            Ok(out)
        })?;

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

    // pub fn download<F>(&self, progress: F)
    // where
    //     F: Fn(PackageKey, u64, u64) -> () + 'static + Send,
    // {

    // }

    pub fn process<F>(&self, progress: F)
    where
        F: Fn(PackageKey, TransactionEvent) -> () + 'static + Send,
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
                                log::error!("{:?}", &e);
                                progress(action.id.clone(), TransactionEvent::Error)
                            }
                        };
                    }
                    PackageActionType::Uninstall => {
                        progress(action.id.clone(), TransactionEvent::Uninstalling);
                        match store.uninstall(&action.id, &action.target) {
                            Ok(_) => progress(action.id.clone(), TransactionEvent::Completed),
                            Err(e) => {
                                log::error!("{:?}", &e);
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
