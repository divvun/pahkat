use std::collections::HashSet;
use std::fmt;
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use url::Url;

use crate::package_store::PackageStore;
use pahkat_types::PackageKey;

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
    InvalidPayload(#[from] crate::repo::PayloadError),
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

use pahkat_types::{
    package::{Descriptor, Release},
    payload::Target,
};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResolvedAction {
    pub action: PackageAction,
    pub descriptor: Descriptor,
    pub release: Release,
    pub target: Target,
}

pub struct PackageTransaction {
    store: Arc<dyn PackageStore>,
    actions: Arc<Vec<ResolvedAction>>,
    is_reboot_required: bool,
}

use crate::repo::PackageCandidateError;

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq, Hash)]
#[serde(rename_all = "camelCase")]
pub struct ResolvedRelease {
    pub version: pahkat_types::package::Version,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub channel: Option<String>,

    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub authors: Vec<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub license: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub license_url: Option<Url>,

    pub target: pahkat_types::payload::Target,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq, Hash)]
pub struct ResolvedDescriptor {
    pub key: PackageKey,
    pub status: PackageStatus,

    pub tags: Vec<String>,
    pub name: pahkat_types::LangTagMap<String>,
    pub description: pahkat_types::LangTagMap<String>,
    pub release: ResolvedRelease,
}

impl ResolvedRelease {
    pub fn new(release: Release, target: Target) -> ResolvedRelease {
        ResolvedRelease {
            version: release.version,
            channel: release.channel,
            authors: release.authors,
            license: release.license,
            license_url: release.license_url,
            target,
        }
    }
}
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq, Hash)]
#[serde(rename_all = "camelCase")]
pub struct ResolvedPackageQuery {
    pub descriptors: Vec<ResolvedDescriptor>,
    pub size: u64,
    pub installed_size: u64,
    pub status: PackageStatus,
}

impl PackageTransaction {
    pub fn new(
        store: Arc<dyn PackageStore>,
        actions: Vec<PackageAction>,
    ) -> Result<PackageTransaction, PackageCandidateError> {
        log::debug!("New transaction with actions: {:#?}", &actions);

        let repos = store.repos();
        let repos = repos.read().unwrap();

        // // Get mutation set (for install and uninstall actions)
        let install_target = actions
            .iter()
            .map(|a| a.target)
            .collect::<std::collections::HashSet<_>>()
            .into_iter()
            .collect::<Vec<_>>();
        let candidate_keys = actions
            .iter()
            .map(|a| (a.action, a.id.clone()))
            .collect::<Vec<_>>();
        let mutation_set = crate::repo::resolve_package_set(
            &*store, &*candidate_keys, &*install_target)?;

        let is_reboot_required = mutation_set.iter().any(|x| x.is_reboot_required);

        // Create a list of resolved actions to be processed.
        let new_actions = mutation_set
            .into_iter()
            .map(|candidate| {
                let key = candidate.package_key;
                let action = candidate.action;

                ResolvedAction {
                    descriptor: candidate.descriptor,
                    release: candidate.release,
                    target: candidate.target,
                    action: actions
                        .iter()
                        .find(|x| &x.id == &key)
                        .cloned()
                        .unwrap_or_else(|| PackageAction {
                            id: key,
                            action,
                            target: InstallTarget::System,
                        }),
                }
            })
            .collect::<Vec<_>>();

        // Check for uninstall actions that contradict this set
        // for action in actions
        //     .iter()
        //     .filter(|x| x.action == PackageActionType::Uninstall)
        // {
        //     if new_actions.iter().any(|x| x.action.id == action.id) {
        //         return Err(PackageCandidateError::UninstallConflict(action.id.clone()));
        //     }
        // }

        log::debug!("Processed actions: {:#?}", &new_actions);

        Ok(PackageTransaction {
            store,
            actions: Arc::new(new_actions),
            is_reboot_required,
        })
    }

    pub fn actions(&self) -> Arc<Vec<ResolvedAction>> {
        Arc::clone(&self.actions)
    }

    pub fn is_reboot_required(&self) -> bool {
        self.is_reboot_required
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
        log::debug!("beginning transaction process NNNNN");

        let stream = async_stream::stream! {
            for record in actions.iter() {
                let action = &record.action;
                log::debug!("processing action: {}", &action);

                match action.action {
                    PackageActionType::Install => {
                        log::debug!("Going to yield now.");
                        yield TransactionEvent::Installing(action.id.clone());

                        log::debug!("Going to install now.");
                        match store.install(&action.id, action.target) {
                            Ok(_) => {
                                log::trace!("We came out the other side.");
                            }
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
