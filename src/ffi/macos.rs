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
use crate::{MacOSPackageStore, PackageKey, StoreConfig};
use crate::repo::RepoRecord;

use super::{JsonMarshaler, PackageKeyMarshaler};

pub type MacOSTarget = pahkat_types::InstallTarget;
pub type MacOSPackageAction = crate::transaction::PackageAction<MacOSTarget>;
pub type MacOSPackageTransaction = crate::transaction::PackageTransaction<MacOSTarget>;

#[cthulhu::invoke(return_marshaler = "cursed::BoxMarshaler::<MacOSPackageStore>")]
pub extern "C" fn pahkat_macos_package_store_default() -> Box<MacOSPackageStore> {
    Box::new(MacOSPackageStore::default())
}

#[cthulhu::invoke(return_marshaler = "cursed::BoxMarshaler::<MacOSPackageStore>")]
pub extern "C" fn pahkat_macos_package_store_new(
    #[marshal(cursed::PathMarshaler)] path: &Path,
) -> Result<Box<MacOSPackageStore>, Box<dyn Error>> {
    let config = StoreConfig::new(&path);
    config.save()?;
    Ok(Box::new(MacOSPackageStore::new(config)))
}

#[cthulhu::invoke(return_marshaler = "cursed::BoxMarshaler::<MacOSPackageStore>")]
pub extern "C" fn pahkat_macos_package_store_load(
    #[marshal(cursed::PathMarshaler)] path: &Path,
) -> Result<Box<MacOSPackageStore>, Box<dyn Error>> {
    let config = match StoreConfig::load(&path, true) {
        Ok(v) => v,
        Err(err) => return Err(Box::new(err) as _),
    };
    Ok(Box::new(MacOSPackageStore::new(config)))
}

#[repr(C)]
#[derive(Debug, Default, Copy, Clone)]
pub struct CPackageStatus {
    is_system: bool,
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

        CPackageStatus { is_system, status }
    }
}

#[cthulhu::invoke(return_marshaler = "cursed::CopyMarshaler::<CPackageStatus>")]
pub extern "C" fn pahkat_macos_package_store_status(
    #[marshal(cursed::ArcRefMarshaler::<MacOSPackageStore>)] handle: Arc<MacOSPackageStore>,
    #[marshal(PackageKeyMarshaler)] package_key: PackageKey,
) -> CPackageStatus {
    handle
        .status(&package_key, &MacOSTarget::User)
        .and_then(|result| Ok(CPackageStatus::new(Ok(result), false)))
        .unwrap_or_else(|_| {
            CPackageStatus::new(handle.status(&package_key, &MacOSTarget::System), true)
        })
}

#[cthulhu::invoke(return_marshaler = "cursed::PathMarshaler")]
pub extern "C" fn pahkat_macos_package_store_download(
    #[marshal(cursed::ArcRefMarshaler::<MacOSPackageStore>)] handle: Arc<MacOSPackageStore>,
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

#[cthulhu::invoke]
pub extern "C" fn pahkat_macos_package_store_clear_cache(
    #[marshal(cursed::ArcRefMarshaler::<MacOSPackageStore>)] handle: Arc<MacOSPackageStore>,
) {
    handle.clear_cache();
}

#[cthulhu::invoke]
pub extern "C" fn pahkat_macos_package_store_refresh_repos(
    #[marshal(cursed::ArcRefMarshaler::<MacOSPackageStore>)] handle: Arc<MacOSPackageStore>,
) {
    handle.refresh_repos();
}

#[cthulhu::invoke]
pub extern "C" fn pahkat_macos_package_store_force_refresh_repos(
    #[marshal(cursed::ArcRefMarshaler::<MacOSPackageStore>)] handle: Arc<MacOSPackageStore>,
) {
    handle.force_refresh_repos();
}

#[cthulhu::invoke(return_marshaler = "cursed::StringMarshaler")]
pub extern "C" fn pahkat_macos_package_store_repo_indexes(
    #[marshal(cursed::ArcRefMarshaler::<MacOSPackageStore>)] handle: Arc<MacOSPackageStore>,
) -> Result<String, Box<dyn Error>> {
    let rwlock = handle.repos();
    let guard = rwlock.read().unwrap();
    let indexes = guard.values().collect::<Vec<&_>>();
    serde_json::to_string(&indexes).map_err(|e| Box::new(e) as _)
}

#[cthulhu::invoke(return_marshaler = "cursed::ArcMarshaler::<RwLock<StoreConfig>>")]
pub extern "C" fn pahkat_macos_package_store_config(
    #[marshal(cursed::ArcRefMarshaler::<MacOSPackageStore>)] handle: Arc<MacOSPackageStore>,
) -> Arc<RwLock<StoreConfig>> {
    handle.config()
}

#[cthulhu::invoke(return_marshaler = "cursed::BoxMarshaler::<MacOSPackageTransaction>")]
pub extern "C" fn pahkat_macos_transaction_new(
    #[marshal(cursed::ArcRefMarshaler::<MacOSPackageStore>)] handle: Arc<MacOSPackageStore>,
    #[marshal(JsonMarshaler)] actions: Vec<MacOSPackageAction>,
) -> Result<Box<MacOSPackageTransaction>, Box<dyn Error>> {
    MacOSPackageTransaction::new(handle as _, actions)
        .map(|x| Box::new(x))
        .map_err(|e| Box::new(e) as _)
}

#[cthulhu::invoke(return_marshaler = "JsonMarshaler")]
pub extern "C" fn pahkat_macos_transaction_actions(
    #[marshal(cursed::BoxRefMarshaler::<MacOSPackageTransaction>)] handle: &MacOSPackageTransaction,
) -> Vec<MacOSPackageAction> {
    handle.actions().to_vec()
}

#[cthulhu::invoke]
pub extern "C" fn pahkat_macos_transaction_process(
    #[marshal(cursed::BoxRefMarshaler::<MacOSPackageTransaction>)] handle: &MacOSPackageTransaction,
    tag: u32,
    progress_callback: extern "C" fn(u32, *const libc::c_char, u32),
) {
    handle.process(move |key, event| {
        let k = PackageKeyMarshaler::to_foreign(&key).unwrap();
        progress_callback(tag, k, event.to_u32());
        // PackageKeyMarshaler::drop_foreign(k);
    })
}
