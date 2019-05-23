use libc::c_char;
use std::ffi::{CString, CStr};
use std::ptr::null;
use serde_json::json;

// #[no_mangle]
// extern fn pahkat_download_package(handle: *const crate::macos::MacOSPackageStore, package_id: *const c_char, progress: extern fn(u64, u64), error: *mut u32) {

// }

// TODO catch unwind!!

use crate::macos::{
    MacOSPackageStore,
    PackageTransaction,
    PackageAction
};
use crate::{
    PackageTransactionError,
    StoreConfig,
    RepoRecord,
    PackageStatus,
    AbsolutePackageKey,
    PackageActionType
};
use pahkat_types::*;
use std::sync::Arc;

#[repr(C)]
struct Repo {
    url: *const c_char,
    channel: *const c_char
}

macro_rules! safe_handle_mut {
    ($handle:ident) => {{
        if $handle.is_null() {
            panic!("handle must not be null");
        }

        unsafe { &mut *$handle }
    }};
}

macro_rules! safe_handle {
    ($handle:ident) => {{
        if $handle.is_null() {
            panic!("handle must not be null");
        }

        unsafe { &*$handle }
    }};
}

#[no_mangle]
extern fn pahkat_client_new(config_path: *const c_char, save_changes: u8) -> *const MacOSPackageStore {
    println!("pahkat_client_new");
    let config = if config_path.is_null() {
        Ok(StoreConfig::load_or_default(save_changes != 0))
    } else {
        let config_path = unsafe { CStr::from_ptr(config_path) }.to_string_lossy();
        StoreConfig::load(std::path::Path::new(&*config_path), save_changes != 0)
    };

    match config {
        Ok(v) => {
            let store = Arc::new(MacOSPackageStore::new(v));
            Arc::into_raw(store)
        }
        Err(_) => std::ptr::null()
    }

    // let repos = config.repos()
    //     .iter()
    //     .map(|record| Repository::from_url(&record.url).unwrap())
    //     .collect::<Vec<_>>();
}

#[no_mangle]
extern fn pahkat_error_free(error: *const *mut PahkatError) {
    println!("pahkat_error_free");
    unsafe { Box::from_raw(*error) };
}

#[no_mangle]
extern fn pahkat_config_path(handle: *const MacOSPackageStore) -> *const c_char {
    let store = safe_handle!(handle);
    let c_str = CString::new(&*store.config().config_path().to_string_lossy()).unwrap();
    CString::into_raw(c_str)
}

#[no_mangle]
extern fn pahkat_config_ui_set(handle: *const MacOSPackageStore, key: *const c_char, value: *const c_char) {
    let store = safe_handle!(handle);
    if key.is_null() {
        return;
    }

    let key = unsafe { CStr::from_ptr(key).to_string_lossy() };
    let value = if value.is_null() {
        None
    } else {
        Some(unsafe { CStr::from_ptr(value).to_string_lossy() })
    };

    store.config().set_ui_setting(&*key, value.map(|x| x.to_string())).unwrap();
}

#[no_mangle]
extern fn pahkat_config_ui_get(handle: *const MacOSPackageStore, key: *const c_char) -> *const c_char {
    let store = safe_handle!(handle);
    if key.is_null() {
        return std::ptr::null();
    }

    let key = unsafe { CStr::from_ptr(key).to_string_lossy() };

    store.config().ui_setting(&*key).map_or_else(|| std::ptr::null(), |x| {
        CString::new(x).unwrap().into_raw()
    })
}

#[no_mangle]
extern fn pahkat_config_repos(handle: *const MacOSPackageStore) -> *const c_char {
    let store = safe_handle!(handle);
    let it = serde_json::to_string(&store.config().repos()).unwrap();
    CString::new(it).unwrap().into_raw()
}

#[no_mangle]
extern fn pahkat_config_set_repos(handle: *const MacOSPackageStore, repos: *const c_char) {
    let store = safe_handle!(handle);
    let repos = unsafe { CStr::from_ptr(repos).to_string_lossy() };
    let repos: Vec<RepoRecord> = serde_json::from_str(&repos).unwrap();
    store.config().set_repos(repos);
}

#[no_mangle]
extern fn pahkat_config_set_cache_path(handle: *const MacOSPackageStore, cache_path: *const c_char) {
    let store = safe_handle!(handle);
    let cache_path = unsafe { CStr::from_ptr(cache_path).to_string_lossy() };
    store.config().set_cache_base_path(std::path::PathBuf::from(&*cache_path));
}

#[no_mangle]
extern fn pahkat_config_cache_path(handle: *const MacOSPackageStore) -> *const c_char {
    let store = safe_handle!(handle);
    CString::new(&*store.config().cache_base_path().to_string_lossy()).unwrap().into_raw()
}

#[no_mangle]
extern fn pahkat_client_free(handle: *const MacOSPackageStore) {
    if handle.is_null() {
        return;
    }
    
    unsafe { Arc::from_raw(safe_handle!(handle)); }
}

#[no_mangle]
extern fn pahkat_str_free(handle: *mut c_char) {
    if handle.is_null() {
        return;
    }

    unsafe { CString::from_raw(safe_handle_mut!(handle)); }
}

#[no_mangle]
extern fn pahkat_repos_json(handle: *const MacOSPackageStore) -> *const c_char {
    let store = safe_handle!(handle);

    let repos = store.repos_json();
    let s = CString::new(&*repos).unwrap().into_raw();

    s
}

#[no_mangle]
extern fn pahkat_refresh_repos(handle: *const MacOSPackageStore) {
    let store = safe_handle!(handle);
    store.refresh_repos();
}

#[no_mangle]
extern fn pahkat_force_refresh_repos(handle: *const MacOSPackageStore) {
    let store = safe_handle!(handle);
    store.force_refresh_repos();
}

struct DownloadPackageKey(*const c_char);
unsafe impl Send for DownloadPackageKey {}

#[no_mangle]
extern fn pahkat_download_package(
    handle: *const MacOSPackageStore,
    raw_package_key: *const c_char,
    target: u8,
    progress: extern fn(*const c_char, u64, u64) -> (),
    error: *mut *const PahkatError
) -> u32 {
    println!("pahkat_download_package");
    let store = safe_handle!(handle);

    if raw_package_key.is_null() {
        let code = ErrorCode::PackageKeyError.to_u32();
        set_error(error, code, "Package key must not be null");
        return code;
    }

    let package_key = unsafe { CStr::from_ptr(raw_package_key) }.to_string_lossy();
    let package_key = AbsolutePackageKey::from_string(&package_key).unwrap();
    let package = match store.resolve_package(&package_key) {
        Some(v) => v,
        None => {
            eprintln!("Resolve package error");
            let code = ErrorCode::PackageResolveError.to_u32();
            set_error(error,
                code,
                &format!("Unable to resolve package {:?}", package_key.to_string())
            );
            return code;
        }
    };

    let download_package_key = DownloadPackageKey(raw_package_key);

    match store.download(&package_key, move |cur, max| {
        progress(download_package_key.0, cur, max);
    }) {
        Ok(_) => 0,
        Err(e) => {
            let code = ErrorCode::PackageDownloadError.to_u32();
            set_error(error,
                code,
                &format!("Unable to download package {:?}", package_key.to_string())
            );
            code
        }
    }
}

#[no_mangle]
extern fn pahkat_status(handle: *const MacOSPackageStore, package_key: *const c_char, error: *mut u32) -> *const c_char {
    // This one is nullable if there's an error.
    let store = safe_handle!(handle);

    if error.is_null() {
        panic!("error must not be null");
    }

    unsafe { *error = 0; }

    fn make_json(status: PackageStatus, target: InstallTarget) -> *const c_char {
        let map = json!({
            "status": status,
            "target": target
        }).to_string();

        CString::new(map)
            .unwrap()
            .into_raw()
    }

    if package_key.is_null() {
        unsafe { *error = 1; }
        return null();
    }

    let package_key = unsafe { CStr::from_ptr(package_key) }.to_string_lossy();
    let package_key = AbsolutePackageKey::from_string(&package_key).unwrap();

    // TODO use package key
    // let package = match store.resolve_package(&package_key) {
    //     Some(v) => v,
    //     None => {
    //         unsafe { *error = 4; }
    //         return null();
    //     }
    // };

    let pkg_status = match store.status(&package_key, InstallTarget::System) {
        Ok(v) => v,
        Err(e) => {
            unsafe { *error = 10; }
            return make_json(PackageStatus::NotInstalled, InstallTarget::System);
        }
    };

    match pkg_status {
        PackageStatus::NotInstalled => {},
        _ => {
            return make_json(pkg_status, InstallTarget::System);
        }
    };

    let pkg_status = match store.status(&package_key, InstallTarget::User) {
        Ok(v) => v,
        Err(e) => {
            unsafe { *error = 10; }
            return make_json(PackageStatus::NotInstalled, InstallTarget::System);
        }
    };

    make_json(pkg_status, InstallTarget::User)
}

#[no_mangle]
extern fn pahkat_create_action(action: u8, target: u8, package_key: *const c_char) -> *mut PackageAction {
    Box::into_raw(Box::new(PackageAction {
        id: AbsolutePackageKey::from_string(&*unsafe { CStr::from_ptr(package_key) }.to_string_lossy()).unwrap(),
        action: PackageActionType::from_u8(action),
        target: if target == 0 { InstallTarget::System } else { InstallTarget::User }
    }))
}

#[no_mangle]
extern fn pahkat_create_package_transaction<'a>(
    handle: *const MacOSPackageStore,
    action_count: u32,
    c_actions: *const *const PackageAction,
    error: *mut *const PahkatError
) -> *const PackageTransaction {
    let store = unsafe { Arc::from_raw(handle) };
    let mut actions = Vec::<PackageAction>::new();

    for i in 0..action_count as isize {
        let ptr = unsafe { *c_actions.offset(i) };
        let action = unsafe { &*ptr }.to_owned();
        actions.push(action);
    }

    let tx = match PackageTransaction::new(store.clone(), actions) {
        Ok(v) => {
            let store = Arc::into_raw(store);
            std::mem::forget(store);
            v
        },
        Err(e) => {
            let c_error = match e {
                PackageTransactionError::NoPackage(id) => {
                    PahkatError {
                        code: 1,
                        message: CString::new(&*format!("No package with id: {}", id)).unwrap().into_raw()
                    }
                }
                PackageTransactionError::Deps(dep_error) => {
                    PahkatError {
                        code: 2,
                        message: CString::new(&*format!("{:?}", dep_error)).unwrap().into_raw()
                    }
                },
                PackageTransactionError::ActionContradiction(id) => {
                    PahkatError {
                        code: 3,
                        message: CString::new(&*format!("Package contradiction for: {}", id)).unwrap().into_raw()
                    }
                }
            };
            unsafe { *error = Box::into_raw(Box::new(c_error)) };
            let store = Arc::into_raw(store);
            std::mem::forget(store);
            return std::ptr::null()
        }
    };

    Box::into_raw(Box::from(tx))
}

#[repr(C)]
#[derive(Debug)]
struct PahkatError {
    pub code: u32,
    pub message: *const c_char
}

impl Drop for PahkatError {
    fn drop(&mut self) {
        unsafe { CString::from_raw(self.message as *mut _) };
    }
}

#[no_mangle]
extern fn pahkat_validate_package_transaction(
    handle: *const MacOSPackageStore,
    transaction: *const PackageTransaction,
    error: *mut *const PahkatError
) -> u32 {
    0
}

#[no_mangle]
extern fn pahkat_run_package_transaction(
    handle: *const MacOSPackageStore,
    transaction: *mut PackageTransaction,
    tx_id: u32,
    progress: extern fn(u32, *const c_char, u32),
    error: *mut *const PahkatError
) -> u32 {
    println!("pahkat_run_package_transaction");
    let transaction = safe_handle_mut!(transaction);

    // TODO: package transaction should also return index of package and total package numbers...
    transaction.process(move |key, event| {
        eprintln!("{:?}", event);
        progress(tx_id, CString::new(key.to_string()).unwrap().into_raw(), event.to_u32())
    });

    0
}

#[no_mangle]
extern fn pahkat_package_transaction_actions(
    handle: *const MacOSPackageStore,
    transaction: *const PackageTransaction,
    error: *mut *const PahkatError
) -> *const c_char {
    let transaction = safe_handle!(transaction);
    
    let json = serde_json::to_string(&*transaction.actions()).expect("serialization issue");
    CString::new(json)
        .unwrap()
        .into_raw()
}

#[no_mangle]
extern fn pahkat_semver_is_valid(version_str: *const c_char) -> u8 {
    let version_string = unsafe { CStr::from_ptr(version_str) }.to_string_lossy();

    match semver::Version::parse(&version_string) {
        Ok(version) => 1,
        _ => {
            eprintln!("pahkat_semver_is_valid: failed to parse version string: {}", &version_string);
            0
        }
    }
}

#[no_mangle]
extern fn pahkat_semver_compare(lhs: *const c_char, rhs: *const c_char) -> i32 {
    let lhs_string = unsafe { CStr::from_ptr(lhs) }.to_string_lossy();
    let rhs_string = unsafe { CStr::from_ptr(rhs) }.to_string_lossy();

    let lhs_version = match semver::Version::parse(&lhs_string) {
        Ok(version) => version,
        _ => {
            eprintln!("pahkat_semver_compare: lhs is not a valid semver");
            return 0
        }
    };
    
    let rhs_version = match semver::Version::parse(&rhs_string) {
        Ok(version) => version,
        _ => {
            eprintln!("pahkat_semver_compare: rhs is not a valid semver");
            return 0
        }
    };

    if lhs_version < rhs_version {
        -1
    } else if lhs_version == rhs_version {
        0
    } else {
        1
    }
}

enum ErrorCode {
    None,
    PackageDownloadError,
    PackageDependencyError,
    PackageActionContradiction,
    PackageResolveError,
    PackageKeyError
}

impl ErrorCode {
    fn to_u32(&self) -> u32 {
        match self {
            ErrorCode::None => 0,
            ErrorCode::PackageDownloadError => 1,
            ErrorCode::PackageDependencyError => 2,
            ErrorCode::PackageActionContradiction => 3,
            ErrorCode::PackageResolveError => 4,
            ErrorCode::PackageKeyError => 5
        }
    }
}

fn set_error(error: *mut *const PahkatError, code: u32, message: &str) {
    let c_message = match CString::new(message) {
        Ok(s) => s,
        Err(_) => CString::new("Failed to create CString representation").unwrap(),
    };

    unsafe {
        if error.is_null() {
            eprintln!("{}", message);
        } else {
            *error = Box::into_raw(Box::new(PahkatError {
                code,
                message: c_message.into_raw()
            }));
        }
    }
}
