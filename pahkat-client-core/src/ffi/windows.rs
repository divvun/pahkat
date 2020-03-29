use std::error::Error;
use std::ffi::{CStr, CString};
use std::path::PathBuf;
use std::sync::{Arc, RwLock};

use cursed::{FromForeign, ToForeign};

use super::{JsonMarshaler, PackageKeyMarshaler};
use crate::package_store::PackageStore;
use crate::transaction::{PackageStatus, PackageStatusError};
use crate::{Config, PackageKey, WindowsPackageStore};

use super::BoxError;

pub type WindowsTarget = pahkat_types::payload::windows::InstallTarget;
pub type WindowsPackageAction = crate::transaction::PackageAction<WindowsTarget>;
pub type WindowsPackageTransaction = crate::transaction::PackageTransaction<WindowsTarget>;

#[cthulhu::invoke(return_marshaler = "cursed::ArcMarshaler::<WindowsPackageStore>")]
pub extern "C" fn pahkat_windows_package_store_default(
) -> Result<Arc<WindowsPackageStore>, Box<dyn Error>> {
    let config = Config::load_default()?;
    Ok(Arc::new(WindowsPackageStore::new(config)))
}

#[cthulhu::invoke(return_marshaler = "cursed::ArcMarshaler::<WindowsPackageStore>")]
pub extern "C" fn pahkat_windows_package_store_new(
    #[marshal(cursed::PathBufMarshaler)] path: PathBuf,
) -> Result<Arc<WindowsPackageStore>, Box<dyn Error>> {
    let config = Config::load(&path, crate::config::Permission::ReadWrite)?;
    Ok(Arc::new(WindowsPackageStore::new(config)))
}

#[cthulhu::invoke(return_marshaler = "cursed::ArcMarshaler::<WindowsPackageStore>")]
pub extern "C" fn pahkat_windows_package_store_load(
    #[marshal(cursed::PathBufMarshaler)] path: PathBuf,
) -> Result<Arc<WindowsPackageStore>, Box<dyn Error>> {
    let config = match Config::load(&path, crate::config::Permission::ReadWrite) {
        Ok(v) => v,
        Err(err) => return Err(Box::new(err) as _),
    };
    Ok(Arc::new(WindowsPackageStore::new(config)))
}

#[repr(C)]
#[derive(Debug, Default, Copy, Clone)]
pub struct CPackageStatus {
    is_system: bool,
    status: i8,
}

impl CPackageStatus {
    fn new(result: Result<PackageStatus, PackageStatusError>, is_system: bool) -> CPackageStatus {
        let status = super::status_to_i8(result);
        CPackageStatus { is_system, status }
    }
}

// #[cthulhu::invoke(return_marshaler = "cursed::CopyMarshaler::<CPackageStatus>")]
// pub extern "C" fn pahkat_windows_package_store_status(
//     #[marshal(cursed::ArcRefMarshaler::<WindowsPackageStore>)] handle: Arc<WindowsPackageStore>,
//     #[marshal(PackageKeyMarshaler)] package_key: PackageKey,
// ) -> CPackageStatus {
//     handle
//         .status(&package_key, &WindowsTarget::User)
//         .and_then(|result| Ok(CPackageStatus::new(Ok(result), false)))
//         .unwrap_or_else(|_| {
//             CPackageStatus::new(handle.status(&package_key, &WindowsTarget::System), true)
//         })
// }

#[cthulhu::invoke]
pub extern "C" fn pahkat_windows_package_store_status(
    #[marshal(cursed::ArcRefMarshaler::<WindowsPackageStore>)] handle: Arc<WindowsPackageStore>,
    #[marshal(PackageKeyMarshaler)] package_key: PackageKey,
    #[marshal(super::TargetMarshaler)] target: WindowsTarget,
) -> i8 {
    super::status_to_i8(handle.status(&package_key, &target))
}

#[cthulhu::invoke(return_marshaler = "cursed::PathBufMarshaler")]
pub extern "C" fn pahkat_windows_package_store_download(
    #[marshal(cursed::ArcRefMarshaler::<WindowsPackageStore>)] handle: Arc<WindowsPackageStore>,
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

#[cthulhu::invoke]
pub extern "C" fn pahkat_windows_package_store_clear_cache(
    #[marshal(cursed::ArcRefMarshaler::<WindowsPackageStore>)] handle: Arc<WindowsPackageStore>,
) {
    handle.clear_cache();
}

#[cthulhu::invoke]
pub extern "C" fn pahkat_windows_package_store_refresh_repos(
    #[marshal(cursed::ArcRefMarshaler::<WindowsPackageStore>)] handle: Arc<WindowsPackageStore>,
) {
    handle.refresh_repos();
}

#[cthulhu::invoke]
pub extern "C" fn pahkat_windows_package_store_force_refresh_repos(
    #[marshal(cursed::ArcRefMarshaler::<WindowsPackageStore>)] handle: Arc<WindowsPackageStore>,
) {
    handle.force_refresh_repos();
}

#[cthulhu::invoke(return_marshaler = "cursed::StringMarshaler")]
pub extern "C" fn pahkat_windows_package_store_repo_indexes(
    #[marshal(cursed::ArcRefMarshaler::<WindowsPackageStore>)] handle: Arc<WindowsPackageStore>,
) -> Result<String, Box<dyn Error>> {
    let rwlock = handle.repos();
    let guard = rwlock.read().unwrap();
    let indexes = guard.values().collect::<Vec<&_>>();
    serde_json::to_string(&indexes).map_err(|e| Box::new(e) as _)
}

#[cthulhu::invoke(return_marshaler = "cursed::ArcMarshaler::<RwLock<Config>>")]
pub extern "C" fn pahkat_windows_package_config(
    #[marshal(cursed::ArcRefMarshaler::<WindowsPackageStore>)] handle: Arc<WindowsPackageStore>,
) -> Arc<RwLock<Config>> {
    handle.config()
}

#[cthulhu::invoke(return_marshaler = "cursed::BoxMarshaler::<WindowsPackageTransaction>")]
pub extern "C" fn pahkat_windows_transaction_new(
    #[marshal(cursed::ArcRefMarshaler::<WindowsPackageStore>)] handle: Arc<WindowsPackageStore>,
    #[marshal(JsonMarshaler)] actions: Vec<WindowsPackageAction>,
) -> Result<Box<WindowsPackageTransaction>, Box<dyn Error>> {
    WindowsPackageTransaction::new(handle as _, actions)
        .map(|x| Box::new(x))
        .map_err(|e| Box::new(e) as _)
}

#[cthulhu::invoke(return_marshaler = "JsonMarshaler")]
pub extern "C" fn pahkat_windows_transaction_actions(
    #[marshal(cursed::BoxRefMarshaler::<WindowsPackageTransaction>)]
    handle: &WindowsPackageTransaction,
) -> Vec<WindowsPackageAction> {
    handle.actions().to_vec()
}

#[cthulhu::invoke(return_marshaler = "cursed::UnitMarshaler")]
pub extern "C" fn pahkat_windows_transaction_process(
    #[marshal(cursed::BoxRefMarshaler::<WindowsPackageTransaction>)]
    handle: &WindowsPackageTransaction,
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
