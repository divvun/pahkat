use std::fmt;
use std::sync::Arc;

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

use crate::repo::PayloadError;

pub fn status_to_i8(result: Result<PackageStatus, PackageStatusError>) -> i8 {
    match result {
        Ok(status) => match status {
            PackageStatus::NotInstalled => 0,
            PackageStatus::UpToDate => 1,
            PackageStatus::RequiresUpdate => 2,
        },
        Err(error) => match error {
            PackageStatusError::Payload(e) => match e {
                PayloadError::NoPackage | PayloadError::NoConcretePackage => -1,
                PayloadError::NoPayloadFound => -2,
                PayloadError::CriteriaUnmet(_) => -5,
            },
            PackageStatusError::WrongPayloadType => -3,
            PackageStatusError::ParsingVersion => -4,
        },
    }
}

impl fmt::Display for PackageStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
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

use crate::package_store::InstallTarget;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub struct PackageAction {
    pub id: PackageKey,
    pub action: PackageActionType,
    #[serde(default)]
    pub target: InstallTarget,
}

impl fmt::Display for PackageAction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PackageAction")
            .field("id", &self.id.to_string())
            .field("action", &self.action)
            .field("target", &self.target)
            .finish()
    }
}

impl PackageAction {
    pub fn install(id: PackageKey, target: InstallTarget) -> PackageAction {
        PackageAction {
            id,
            action: PackageActionType::Install,
            target,
        }
    }

    pub fn uninstall(id: PackageKey, target: InstallTarget) -> PackageAction {
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

#[derive(Debug, Clone, thiserror::Error)]
pub enum PackageDependencyError {
    #[error("Package not found: {0}")]
    PackageNotFound(String),

    #[error("Version not found: {0}")]
    VersionNotFound(String),

    #[error("A package status was invalid: {0}")]
    PackageStatusError(String, #[source] PackageStatusError),
}

#[derive(Debug, Clone, thiserror::Error)]
pub enum PackageTransactionError {
    #[error("No package found with identifier: {0}")]
    NoPackage(String),

    #[error("A dependency resolution error occurred")]
    Deps(#[from] PackageDependencyError),

    #[error("Some transaction actions contradict: {0}")]
    ActionContradiction(String),

    #[error("Invalid package status detected")]
    InvalidStatus(#[from] crate::transaction::PackageStatusError),
    
    #[error("A payload could not be resolved")]
    InvalidPayload(#[from] crate::repo::PayloadError)
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

fn process_install_action(
    store: &Arc<dyn PackageStore>,
    package: &Descriptor,
    action: &PackageAction,
    new_actions: &mut Vec<ResolvedAction>,
) -> Result<(), PackageTransactionError> {
    let dependencies =
        match crate::repo::find_package_dependencies(store, &action.id, package, &action.target) {
            Ok(d) => d,
            Err(e) => return Err(PackageTransactionError::Deps(e)),
        };

    // for dependency in dependencies.into_iter() {
    //     if !new_actions.iter().any(|x| x.id == dependency.0) {
    //         // TODO: validate that it is allowed for user installations
    //         let new_action = PackageAction::install(dependency.0, action.target.clone());
    //         new_actions.push(new_action);
    //     }
    // }

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

#[derive(Debug)]
pub enum TransactionEvent {
    Installing(PackageKey),
    Uninstalling(PackageKey),
    Progress(PackageKey, String),
    Error(PackageKey, TransactionError),
    Complete,
}

use pahkat_types::{payload::Target, package::{Descriptor, Release}};

#[derive(Debug, Clone)]
pub struct ResolvedAction {
    pub action: PackageAction,
    pub descriptor: Descriptor,
    pub release: Release,
    pub target: Target,
}

pub struct PackageTransaction {
    store: Arc<dyn PackageStore>,
    actions: Arc<Vec<ResolvedAction>>,
}

impl PackageTransaction {
    pub fn new(
        store: Arc<dyn PackageStore>,
        actions: Vec<PackageAction>,
    ) -> Result<PackageTransaction, PackageTransactionError> {
        let mut new_actions: Vec<ResolvedAction> = vec![];

        log::debug!("New transaction with actions: {:#?}", &actions);

        let repos = store.repos();
        let repos = repos.read().unwrap();

        // Collate all dependencies
        for action in actions.into_iter() {
            let package_key = &action.id;
            let query = crate::repo::ReleaseQuery::new(package_key, &*repos);

            let (target, release, descriptor) = crate::repo::resolve_payload(package_key, &query, &*repos)?;

            // let package = store
            //     .find_package_by_key(&package_key)
            //     .ok_or_else(|| PackageTransactionError::NoPackage(package_key.to_string()))?;

            if action.is_install() {
                // Add all sub-dependencies
                process_install_action(&store, &descriptor, &action, &mut new_actions)?;
            }

            // if let Some(found_action) = new_actions.iter().find(|x| x.id == action.id) {
            //     if found_action.action != action.action {
            //         return Err(PackageTransactionError::ActionContradiction(
            //             action.id.to_string(),
            //         ));
            //     }
            // } else {
            //     new_actions.push(action);
            // }
        }

        // Check for contradictions
        let mut installs = HashSet::new();
        let mut uninstalls = HashSet::new();

        for record in new_actions.iter() {
            if record.action.is_install() {
                installs.insert(&record.action.id);
            } else {
                uninstalls.insert(&record.action.id);
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
        // let new_actions: Vec<ResolvedAction> = new_actions
        //     .into_iter()
        //     .try_fold(Vec::<ResolvedAction>::new(), |mut out, record| {
        //         let status = store
        //             .status(&record.action.id, record.action.target)
        //             .map_err(|err| PackageTransactionError::InvalidStatus(err))?;

        //         let is_valid = if record.action.is_install() {
        //             status != PackageStatus::UpToDate
        //         } else {
        //             status == PackageStatus::UpToDate || status == PackageStatus::RequiresUpdate
        //         };

        //         if is_valid {
        //             out.push(record);
        //         }

        //         Ok(out)
        //     })?;

        log::debug!("Processed actions: {:#?}", &new_actions);

        Ok(PackageTransaction {
            store,
            actions: Arc::new(new_actions),
        })
    }

    pub fn actions(&self) -> Arc<Vec<ResolvedAction>> {
        Arc::clone(&self.actions)
    }

    pub fn validate(&self) -> bool {
        true
    }

    pub fn process(
        &self,
    ) -> (
        stream_cancel::Trigger,
        crate::package_store::Stream<TransactionEvent>,
    ) {
        log::debug!("beginning transaction process");

        let (canceler, valve) = stream_cancel::Valve::new();

        let store = Arc::clone(&self.store);
        let actions: Arc<Vec<ResolvedAction>> = Arc::clone(&self.actions);

        let stream = async_stream::stream! {
            for record in actions.iter() {
                let action = &record.action;
                log::debug!("processing action: {}", &action);

                match action.action {
                    PackageActionType::Install => {
                        yield TransactionEvent::Installing(action.id.clone());

                        match store.install(&action.id, action.target) {
                            Ok(_) => {}
                            Err(e) => {
                                log::error!("{:?}", &e);
                                yield TransactionEvent::Error(action.id.clone(), TransactionError::Install(e));
                                return;
                            }
                        };
                    }
                    PackageActionType::Uninstall => {
                        yield TransactionEvent::Uninstalling(action.id.clone());

                        match store.uninstall(&action.id, action.target) {
                            Ok(_) => {}
                            Err(e) => {
                                log::error!("{:?}", &e);
                                yield TransactionEvent::Error(action.id.clone(), TransactionError::Uninstall(e));
                                return;
                            }
                        };
                    }
                }
            }

            yield TransactionEvent::Complete;
        };

        (canceler, Box::pin(valve.wrap(stream)))
    }
}
