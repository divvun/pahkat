use std::fmt;
use std::sync::Arc;
use std::thread::JoinHandle;

use pahkat_types::package::Package;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

use crate::package_store::PackageStore;
use crate::PackageKey;

pub mod install;
pub mod uninstall;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum PackageStatus {
    NotInstalled,
    UpToDate,
    RequiresUpdate,
}

impl fmt::Display for PackageStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(
            f,
            "{}",
            match *self {
                PackageStatus::NotInstalled => "Not installed",
                PackageStatus::UpToDate => "Up to date",
                PackageStatus::RequiresUpdate => "Requires update",
            }
        )
    }
}

pub trait PackageTarget: Send + Sync + Clone {}

/// This is so good.
impl PackageTarget for () {}
impl PackageTarget for pahkat_types::payload::windows::InstallTarget {}
impl PackageTarget for pahkat_types::payload::macos::InstallTarget {}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PackageAction<T: PackageTarget> {
    pub id: PackageKey,
    pub action: PackageActionType,
    pub target: T,
}

impl<T: fmt::Debug + PackageTarget> fmt::Display for PackageAction<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct(&format!("PackageAction<{}>", std::any::type_name::<T>()))
            .field("id", &self.id.to_string())
            .field("action", &self.action)
            .field("target", &self.target)
            .finish()
    }
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

#[derive(Debug, thiserror::Error, Clone)]
pub enum PackageStatusError {
    #[error("Payload error")]
    Payload(#[from] crate::repo::PayloadError),

    #[error("Wrong payload type")]
    WrongPayloadType,

    #[error("Error parsing version")]
    ParsingVersion,
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

#[derive(Debug)]
pub enum TransactionEvent {
    Uninstalling,
    Installing,
}

impl TransactionEvent {
    pub fn to_u32(&self) -> u32 {
        match self {
            TransactionEvent::Uninstalling => 1,
            TransactionEvent::Installing => 2,
        }
    }
}

#[derive(Debug, Clone)]
pub enum PackageTransactionError {
    NoPackage(String),
    Deps(PackageDependencyError),
    ActionContradiction(String),
    InvalidStatus(crate::transaction::PackageStatusError),
}

impl std::error::Error for PackageTransactionError {}

impl fmt::Display for PackageTransactionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "{:?}", self)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
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

use self::install::InstallError;
use self::uninstall::UninstallError;

#[derive(Debug)]
pub enum TransactionError {
    ValidationFailed,
    UserCancelled,
    Uninstall(UninstallError),
    Install(InstallError),
}

impl std::error::Error for TransactionError {}

impl std::fmt::Display for TransactionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        use TransactionError::*;

        match self {
            ValidationFailed => write!(f, "Validation failed"),
            UserCancelled => write!(f, "User cancelled"),
            Uninstall(e) => write!(f, "{:?}", e),
            Install(e) => write!(f, "{:?}", e),
        }
    }
}

pub struct PackageTransaction<T: PackageTarget + std::fmt::Debug + 'static> {
    store: Arc<dyn PackageStore<Target = T>>,
    actions: Arc<Vec<PackageAction<T>>>,
}

impl<T: PackageTarget + std::fmt::Debug + 'static> PackageTransaction<T> {
    pub fn new(
        store: Arc<dyn PackageStore<Target = T>>,
        actions: Vec<PackageAction<T>>,
    ) -> Result<PackageTransaction<T>, PackageTransactionError> {
        let mut new_actions: Vec<PackageAction<T>> = vec![];

        log::debug!("New transaction with actions: {:#?}", &actions);

        // Collate all dependencies
        for action in actions.into_iter() {
            let package_key = &action.id;

            let package = store
                .find_package_by_key(&package_key)
                .ok_or_else(|| PackageTransactionError::NoPackage(package_key.to_string()))?;

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
            return Err(PackageTransactionError::ActionContradiction(format!(
                "{:?}",
                contradictions
            )));
        }

        // Check if packages need to even be installed or uninstalled
        let new_actions = new_actions
            .into_iter()
            .try_fold(vec![], |mut out, action| {
                let status = store
                    .status(&action.id, &action.target as _)
                    .map_err(|err| PackageTransactionError::InvalidStatus(err))?;

                let is_valid = if action.is_install() {
                    status != PackageStatus::UpToDate
                } else {
                    status == PackageStatus::UpToDate || status == PackageStatus::RequiresUpdate
                };

                if is_valid {
                    out.push(action);
                }

                Ok(out)
            })?;

        log::debug!("Processed actions: {:#?}", &new_actions);

        Ok(PackageTransaction {
            store,
            actions: Arc::new(new_actions),
        })
    }

    pub fn actions(&self) -> Arc<Vec<PackageAction<T>>> {
        Arc::clone(&self.actions)
    }

    pub fn validate(&self) -> bool {
        true
    }

    pub fn process<F>(&self, progress: F) -> JoinHandle<Result<(), TransactionError>>
    where
        F: Fn(PackageKey, TransactionEvent) -> bool + 'static + Send,
    {
        log::debug!("beginning transaction process");
        let is_valid = self.validate();
        let store = Arc::clone(&self.store);
        let actions: Arc<Vec<PackageAction<T>>> = Arc::clone(&self.actions);

        std::thread::spawn(move || {
            if !is_valid {
                // TODO: early return
                return Err(TransactionError::ValidationFailed);
            }

            let mut is_cancelled = false;

            for action in actions.iter() {
                log::debug!("processing action: {}", &action);

                if is_cancelled {
                    return Err(TransactionError::UserCancelled);
                }

                match action.action {
                    PackageActionType::Install => {
                        is_cancelled = !progress(action.id.clone(), TransactionEvent::Installing);
                        match store.install(&action.id, &action.target) {
                            Ok(_) => {}
                            Err(e) => {
                                log::error!("{:?}", &e);
                                return Err(TransactionError::Install(e));
                            }
                        };
                    }
                    PackageActionType::Uninstall => {
                        is_cancelled = !progress(action.id.clone(), TransactionEvent::Uninstalling);
                        match store.uninstall(&action.id, &action.target) {
                            Ok(_) => {}
                            Err(e) => {
                                log::error!("{:?}", &e);
                                return Err(TransactionError::Uninstall(e));
                            }
                        };
                    }
                }
            }

            Ok(())
        })
    }
}
