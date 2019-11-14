use std::convert::TryFrom;
use std::error::Error;
use std::ffi::{CStr, CString};
use std::marker::PhantomData;
use std::path::{Path, PathBuf};
use std::sync::{Arc, RwLock};

use cursed::{FromForeign, InputType, ReturnType, ToForeign};
use serde::de::DeserializeOwned;
use serde::Serialize;

use crate::package_store::PackageStore;
use crate::transaction::{PackageAction, PackageStatus, PackageStatusError, PackageTransactionError};
use crate::{PrefixPackageStore, PackageKey, StoreConfig};
use crate::repo::RepoRecord;

use super::{JsonMarshaler, PackageKeyMarshaler};

pub type PrefixTarget = ();
pub type PrefixPackageAction = crate::transaction::PackageAction<PrefixTarget>;
pub type PrefixPackageTransaction = crate::transaction::PackageTransaction<PrefixTarget>;

#[cthulhu::invoke(return_marshaler = "cursed::ArcMarshaler::<PrefixPackageStore>")]
pub extern "C" fn pahkat_prefix_package_store_open(
    #[marshal(cursed::PathMarshaler)] prefix_path: &Path,
) -> Result<Arc<PrefixPackageStore>, Box<dyn Error>> {
    PrefixPackageStore::open(prefix_path).map(|x| Arc::new(x))
}

#[cthulhu::invoke(return_marshaler = "cursed::ArcMarshaler::<PrefixPackageStore>")]
pub extern "C" fn pahkat_prefix_package_store_create(
    #[marshal(cursed::PathMarshaler)] prefix_path: &Path,
) -> Result<Arc<PrefixPackageStore>, Box<dyn Error>> {
    PrefixPackageStore::create(prefix_path).map(|x| Arc::new(x))
}

#[repr(C)]
#[derive(Debug, Default, Copy, Clone)]
pub struct CPackageStatus {
    is_system: u8,
    status: i8,
}

impl CPackageStatus {
    fn new(result: Result<PackageStatus, PackageStatusError>, is_system: bool) -> CPackageStatus {
        use PackageStatusError::*;

        let status = match result {
            Ok(status) => match status {
                PackageStatus::NotInstalled => 0,
                PackageStatus::UpToDate => 1,
                PackageStatus::RequiresUpdate => 2,
                PackageStatus::Skipped => 3,
            },
            Err(error) => match error {
                NoPackage => -1,
                NoInstaller => -2,
                WrongInstallerType => -3,
                ParsingVersion => -4,
                InvalidInstallPath => -5,
                InvalidMetadata => -6,
            },
        };

        let is_system = if is_system { 1 } else { 0 };

        CPackageStatus { is_system, status }
    }
}

#[cthulhu::invoke(return_marshaler = "cursed::CopyMarshaler::<CPackageStatus>")]
pub extern "C" fn pahkat_prefix_package_store_status(
    #[marshal(cursed::ArcRefMarshaler::<PrefixPackageStore>)] handle: Arc<PrefixPackageStore>,
    #[marshal(PackageKeyMarshaler)] package_key: PackageKey,
) -> CPackageStatus {
    CPackageStatus::new(handle.status(&package_key, &()), true)
}

#[cthulhu::invoke(return_marshaler = "cursed::PathMarshaler")]
pub extern "C" fn pahkat_prefix_package_store_download(
    #[marshal(cursed::ArcRefMarshaler::<PrefixPackageStore>)] handle: Arc<PrefixPackageStore>,
    #[marshal(PackageKeyMarshaler)] package_key: PackageKey,
    progress: extern "C" fn(*const PackageKey, u64, u64),
) -> Result<PathBuf, Box<dyn Error>> {
    let package_key1 = package_key.to_owned();
    handle
        .download(
            &package_key,
            Box::new(move |cur, max| {
                progress(&package_key1 as *const _, cur, max);
            }),
        )
        .map_err(|e| Box::new(e) as _)
}

#[cthulhu::invoke(return_marshaler = "JsonMarshaler")]
pub extern "C" fn pahkat_prefix_package_store_resolve_package(
    #[marshal(cursed::ArcRefMarshaler::<PrefixPackageStore>)] handle: Arc<PrefixPackageStore>,
    #[marshal(PackageKeyMarshaler)] package_key: PackageKey,
) -> Option<pahkat_types::Package> {
    handle.resolve_package(&package_key)
}

#[cthulhu::invoke]
pub extern "C" fn pahkat_prefix_package_store_clear_cache(
    #[marshal(cursed::ArcRefMarshaler::<PrefixPackageStore>)] handle: Arc<PrefixPackageStore>,
) {
    handle.clear_cache();
}

#[cthulhu::invoke]
pub extern "C" fn pahkat_prefix_package_store_refresh_repos(
    #[marshal(cursed::ArcRefMarshaler::<PrefixPackageStore>)] handle: Arc<PrefixPackageStore>,
) {
    handle.refresh_repos();
}

#[cthulhu::invoke]
pub extern "C" fn pahkat_prefix_package_store_force_refresh_repos(
    #[marshal(cursed::ArcRefMarshaler::<PrefixPackageStore>)] handle: Arc<PrefixPackageStore>,
) {
    handle.force_refresh_repos();
}

#[cthulhu::invoke(return_marshaler = "cursed::StringMarshaler")]
pub extern "C" fn pahkat_prefix_package_store_repo_indexes(
    #[marshal(cursed::ArcRefMarshaler::<PrefixPackageStore>)] handle: Arc<PrefixPackageStore>,
) -> Result<String, Box<dyn Error>> {
    let rwlock = handle.repos();
    let guard = rwlock.read().unwrap();
    let indexes = guard.values().collect::<Vec<&_>>();
    serde_json::to_string(&indexes).map_err(|e| Box::new(e) as _)
}

#[cthulhu::invoke(return_marshaler = "cursed::ArcMarshaler::<RwLock<StoreConfig>>")]
pub extern "C" fn pahkat_prefix_package_store_config(
    #[marshal(cursed::ArcRefMarshaler::<PrefixPackageStore>)] handle: Arc<PrefixPackageStore>,
) -> Arc<RwLock<StoreConfig>> {
    handle.config()
}

#[cthulhu::invoke(return_marshaler = "cursed::BoxMarshaler::<PrefixPackageTransaction>")]
pub extern "C" fn pahkat_prefix_transaction_new(
    #[marshal(cursed::ArcRefMarshaler::<PrefixPackageStore>)] handle: Arc<PrefixPackageStore>,
    #[marshal(JsonMarshaler)] actions: Vec<PrefixPackageAction>,
) -> Result<Box<PrefixPackageTransaction>, Box<dyn Error>> {
    PrefixPackageTransaction::new(handle as _, actions)
        .map(|x| Box::new(x))
        .map_err(|e| Box::new(e) as _)
}

#[cthulhu::invoke(return_marshaler = "JsonMarshaler")]
pub extern "C" fn pahkat_prefix_transaction_actions(
    #[marshal(cursed::BoxRefMarshaler::<PrefixPackageTransaction>)] handle: &PrefixPackageTransaction,
) -> Vec<PrefixPackageAction> {
    handle.actions().to_vec()
}

#[cthulhu::invoke]
pub extern "C" fn pahkat_prefix_transaction_process(
    #[marshal(cursed::BoxRefMarshaler::<PrefixPackageTransaction>)] handle: &PrefixPackageTransaction,
    tag: u32,
    progress_callback: extern "C" fn(u32, *const libc::c_char, u32),
) {
    handle.process(move |key, event| {
        let k = PackageKeyMarshaler::to_foreign(&key).unwrap();
        progress_callback(tag, k, event.to_u32());
        // PackageKeyMarshaler::drop_foreign(k);
    })
}
