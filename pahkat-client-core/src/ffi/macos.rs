use std::collections::BTreeMap;
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
use crate::repo::RepoRecord;
use crate::transaction::{
    PackageAction, PackageStatus, PackageStatusError, PackageTransactionError,
};
use crate::{MacOSPackageStore, PackageKey, StoreConfig};

use super::{JsonMarshaler, PackageKeyMarshaler, TargetMarshaler};

pub type MacOSTarget = pahkat_types::InstallTarget;
pub type MacOSPackageAction = crate::transaction::PackageAction<MacOSTarget>;
pub type MacOSPackageTransaction = crate::transaction::PackageTransaction<MacOSTarget>;

// #[cffi::marshal(return_marshaler = "cursed::ArcMarshaler::<MacOSPackageStore>")]
// pub extern "C" fn pahkat_macos_package_store_default() -> Arc<MacOSPackageStore> {
//     Arc::new(MacOSPackageStore::default())
// }

#[cffi::marshal(return_marshaler = "cursed::ArcMarshaler::<MacOSPackageStore>")]
pub extern "C" fn pahkat_macos_package_store_new(
    #[marshal(cursed::PathBufMarshaler)] path: PathBuf,
) -> Result<Arc<MacOSPackageStore>, Box<dyn Error>> {
    let config = StoreConfig::new(&path);
    config.save()?;
    Ok(Arc::new(MacOSPackageStore::new(config)))
}

#[cffi::marshal(return_marshaler = "cursed::ArcMarshaler::<MacOSPackageStore>")]
pub extern "C" fn pahkat_macos_package_store_load(
    #[marshal(cursed::PathBufMarshaler)] path: PathBuf,
) -> Result<Arc<MacOSPackageStore>, Box<dyn Error>> {
    let config = match StoreConfig::load(&path, true) {
        Ok(v) => v,
        Err(err) => return Err(Box::new(err) as _),
    };
    Ok(Arc::new(MacOSPackageStore::new(config)))
}

#[cffi::marshal]
pub extern "C" fn pahkat_macos_package_store_status(
    #[marshal(cursed::ArcRefMarshaler::<MacOSPackageStore>)] handle: Arc<MacOSPackageStore>,
    #[marshal(PackageKeyMarshaler)] package_key: PackageKey,
    #[marshal(TargetMarshaler)] target: MacOSTarget,
) -> i8 {
    super::status_to_i8(handle.status(&package_key, &target))
}

#[cffi::marshal(return_marshaler = "JsonMarshaler")]
pub extern "C" fn pahkat_macos_package_store_all_statuses(
    #[marshal(cursed::ArcRefMarshaler::<MacOSPackageStore>)] handle: Arc<MacOSPackageStore>,
    #[marshal(JsonMarshaler)] repo_record: RepoRecord,
    #[marshal(TargetMarshaler)] target: MacOSTarget,
) -> BTreeMap<String, i8> {
    let statuses = handle.all_statuses(&repo_record, &target);
    statuses
        .into_iter()
        .map(|(id, result)| (id, super::status_to_i8(result)))
        .collect()
}

#[cffi::marshal(return_marshaler = "cursed::PathBufMarshaler")]
pub extern "C" fn pahkat_macos_package_store_import(
    #[marshal(cursed::ArcRefMarshaler::<MacOSPackageStore>)] handle: Arc<MacOSPackageStore>,
    #[marshal(PackageKeyMarshaler)] package_key: PackageKey,
    #[marshal(cursed::PathBufMarshaler)] installer_path: PathBuf,
) -> Result<PathBuf, Box<dyn Error>> {
    handle.import(&package_key, &installer_path)
}

#[cffi::marshal(return_marshaler = "cursed::PathBufMarshaler")]
pub extern "C" fn pahkat_macos_package_store_download(
    #[marshal(cursed::ArcRefMarshaler::<MacOSPackageStore>)] handle: Arc<MacOSPackageStore>,
    #[marshal(PackageKeyMarshaler)] package_key: PackageKey,
    progress: extern "C" fn(*const libc::c_char, u64, u64) -> u8,
) -> Result<PathBuf, Box<dyn Error>> {
    let package_key_str = CString::new(package_key.to_string()).unwrap();
    handle
        .download(
            &package_key,
            Box::new(move |cur, max| progress(package_key_str.as_ptr(), cur, max) != 0),
        )
        .map_err(|e| Box::new(e) as _)
}

#[cffi::marshal(return_marshaler = "JsonMarshaler")]
pub extern "C" fn pahkat_macos_package_store_find_package_by_key(
    #[marshal(cursed::ArcRefMarshaler::<MacOSPackageStore>)] handle: Arc<MacOSPackageStore>,
    #[marshal(PackageKeyMarshaler)] package_key: PackageKey,
) -> Option<pahkat_types::Package> {
    handle.find_package_by_key(&package_key)
}

#[cffi::marshal]
pub extern "C" fn pahkat_macos_package_store_clear_cache(
    #[marshal(cursed::ArcRefMarshaler::<MacOSPackageStore>)] handle: Arc<MacOSPackageStore>,
) {
    handle.clear_cache();
}

#[cffi::marshal]
pub extern "C" fn pahkat_macos_package_store_refresh_repos(
    #[marshal(cursed::ArcRefMarshaler::<MacOSPackageStore>)] handle: Arc<MacOSPackageStore>,
) {
    handle.refresh_repos();
}

#[cffi::marshal]
pub extern "C" fn pahkat_macos_package_store_force_refresh_repos(
    #[marshal(cursed::ArcRefMarshaler::<MacOSPackageStore>)] handle: Arc<MacOSPackageStore>,
) {
    handle.force_refresh_repos();
}

#[cffi::marshal(return_marshaler = "cursed::StringMarshaler")]
pub extern "C" fn pahkat_macos_package_store_repo_indexes(
    #[marshal(cursed::ArcRefMarshaler::<MacOSPackageStore>)] handle: Arc<MacOSPackageStore>,
) -> Result<String, Box<dyn Error>> {
    let rwlock = handle.repos();
    let guard = rwlock.read().unwrap();
    let indexes = guard.values().collect::<Vec<&_>>();
    serde_json::to_string(&indexes).map_err(|e| Box::new(e) as _)
}

#[cffi::marshal(return_marshaler = "cursed::ArcMarshaler::<RwLock<StoreConfig>>")]
pub extern "C" fn pahkat_macos_package_config(
    #[marshal(cursed::ArcRefMarshaler::<MacOSPackageStore>)] handle: Arc<MacOSPackageStore>,
) -> Arc<RwLock<StoreConfig>> {
    handle.config()
}

#[cffi::marshal(return_marshaler = "cursed::BoxMarshaler::<MacOSPackageTransaction>")]
pub extern "C" fn pahkat_macos_transaction_new(
    #[marshal(cursed::ArcRefMarshaler::<MacOSPackageStore>)] handle: Arc<MacOSPackageStore>,
    #[marshal(JsonMarshaler)] actions: Vec<MacOSPackageAction>,
) -> Result<Box<MacOSPackageTransaction>, Box<dyn Error>> {
    MacOSPackageTransaction::new(handle as _, actions)
        .map(|x| Box::new(x))
        .map_err(|e| Box::new(e) as _)
}

#[cffi::marshal(return_marshaler = "JsonMarshaler")]
pub extern "C" fn pahkat_macos_transaction_actions(
    #[marshal(cursed::BoxRefMarshaler::<MacOSPackageTransaction>)] handle: &MacOSPackageTransaction,
) -> Vec<MacOSPackageAction> {
    handle.actions().to_vec()
}

#[cffi::marshal(return_marshaler = "cursed::UnitMarshaler")]
pub extern "C" fn pahkat_macos_transaction_process(
    #[marshal(cursed::BoxRefMarshaler::<MacOSPackageTransaction>)] handle: &MacOSPackageTransaction,
    tag: u32,
    progress_callback: extern "C" fn(u32, *const libc::c_char, u32) -> u8,
) -> Result<(), Box<dyn Error>> {
    handle
        .process(move |key, event| {
            let k = PackageKeyMarshaler::to_foreign(&key).unwrap();
            progress_callback(tag, k, event.to_u32()) != 0
            // PackageKeyMarshaler::drop_foreign(k);
        })
        .join()
        .unwrap()
        .map_err(|e| Box::new(e) as _)
}
