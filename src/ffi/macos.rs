use libc::c_char;
use std::ffi::{CString, CStr};
use std::ptr::null;
use serde_json::json;

// #[no_mangle]
// extern fn pahkat_download_package(handle: *const crate::macos::MacOSPackageStore, package_id: *const c_char, progress: extern fn(u64, u64), error: *mut u32) {

// }

// TODO catch unwind!!

use crate::macos::MacOSPackageStore;
use crate::macos::PackageTransaction;
use crate::StoreConfig;
use crate::RepoRecord;
use crate::repo::{PackageRecord, Repository};
use crate::PackageStatus;
use crate::AbsolutePackageKey;
use pahkat::types::*;
use crate::macos::{PackageAction, PackageActionType};
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
extern fn pahkat_client_new(config_path: *const c_char) -> *const MacOSPackageStore {
    let config = if config_path.is_null() {
        Ok(StoreConfig::load_or_default())
    } else {
        let config_path = unsafe { CStr::from_ptr(config_path) }.to_string_lossy();
        StoreConfig::load(std::path::Path::new(&*config_path))
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
// extern uint32_t /* error */
// pahkat_download_package(const pahkat_client_t* _Nonnull handle,
//     const char* package_key,
//     uint8_t target,
//     void (*progress)(const char* /* package_id */, uint64_t /* cur */, uint64_t /* max */));

struct DownloadPackageKey(*const c_char);
unsafe impl Send for DownloadPackageKey {}

#[no_mangle]
extern fn pahkat_download_package(
    handle: *const MacOSPackageStore,
    package_key: *const c_char,
    target: u8,
    progress: extern fn(*const c_char, u64, u64) -> (),
    error: *mut *const PahkatError
) -> u32 {
    println!("Called into FFI");
    let store = safe_handle!(handle);

    if package_key.is_null() {
        let code = ErrorCode::PackageKeyError.to_u32();
        set_error(error, code, "Package key must not be null");
        return code;
    }

    let package_id = unsafe { CStr::from_ptr(package_key) }.to_string_lossy();
    let package_id = AbsolutePackageKey::from_string(&package_id).unwrap();
    println!("Package id: {:?}", package_id);
    let package = match store.resolve_package(&package_id) {
        Some(v) => v,
        None => {
            let code = ErrorCode::PackageResolveError.to_u32();
            set_error(error,
                code,
                &format!("Unable to resolve package {:?}", package_id.to_string())
            );
            return code;
        }
    };

    println!("Gonna download");
    let download_package_key = DownloadPackageKey(package_key);

    match store.download(&package, move |cur, max| {
        println!("{}/{}", cur, max);
        progress(download_package_key.0, cur, max);
    }) {
        Ok(_) => 0,
        Err(e) => {
            let code = ErrorCode::PackageDownloadError.to_u32();
            set_error(error,
                code,
                &format!("Unable to download package {:?}", package_id.to_string())
            );
            code
        }
    }
}

#[no_mangle]
extern fn pahkat_status(handle: *const MacOSPackageStore, package_id: *const c_char, error: *mut u32) -> *const c_char {
    // This one is nullable if there's an error.
    let store = safe_handle!(handle);

    if error.is_null() {
        panic!("error must not be null");
    }

    unsafe { *error = 0; }

    fn make_json(status: PackageStatus, target: MacOSInstallTarget) -> *const c_char {
        let map = json!({
            "status": status,
            "target": target
        }).to_string();

        CString::new(map)
            .unwrap()
            .into_raw()
    }

    if package_id.is_null() {
        unsafe { *error = 1; }
        return null();
    }

    let package_id = unsafe { CStr::from_ptr(package_id) }.to_string_lossy();
    // TODO use package key
    let package = match store.find_package(&package_id) {
        Some(v) => v,
        None => {
            unsafe { *error = 4; }
            return null();
        }
    };

    let pkg_status = match store.status(&package, MacOSInstallTarget::System) {
        Ok(v) => v,
        Err(e) => {
            unsafe { *error = 10; }
            return make_json(PackageStatus::NotInstalled, MacOSInstallTarget::System);
        }
    };

    match pkg_status {
        PackageStatus::NotInstalled => {},
        _ => {
            return make_json(pkg_status, MacOSInstallTarget::System);
        }
    };

    let pkg_status = match store.status(&package, MacOSInstallTarget::User) {
        Ok(v) => v,
        Err(e) => {
            unsafe { *error = 10; }
            return make_json(PackageStatus::NotInstalled, MacOSInstallTarget::System);
        }
    };

    make_json(pkg_status, MacOSInstallTarget::User)
}
// typedef struct pahkat_action_s {
//     const uint8_t action;
//     const uint8_t target;
//     const char* _Nonnull package_key;
// } pahkat_action_t;
// extern pahkat_action_t*
// pahkat_create_action(uint8_t action, uint8_t target, const char* _Nonnull package_key);

#[repr(C)]
struct CPackageAction {
    pub action: u8,
    pub target: u8,
    pub package_key: *mut c_char
}

impl Drop for CPackageAction {
    fn drop(&mut self) {
        unsafe { CString::from_raw(self.package_key) };
    }
}


impl CPackageAction {
    pub fn new(action: u8, target: u8, package_key: *const c_char) -> CPackageAction {
        let package_key = unsafe { CString::from(CStr::from_ptr(package_key)).into_raw() };
        CPackageAction { action, target, package_key }
    }
}

#[no_mangle]
extern fn pahkat_create_action(action: u8, target: u8, package_key: *const c_char) -> *mut CPackageAction {
    Box::into_raw(Box::new(CPackageAction::new(action, target, package_key)))
}


// struct PackageAction {
//     package: PackageRecord,
//     action: PackageActionType,
//     target: MacOSInstallTarget
// }

#[no_mangle]
extern fn pahkat_create_package_transaction<'a>(
    handle: *const MacOSPackageStore,
    action_count: u32,
    c_actions: *const CPackageAction,
    error: *mut *const PahkatError
) -> *const PackageTransaction {
    println!("Action count: {}", action_count);

    let store = unsafe { Arc::from_raw(handle) };
    let mut actions = Vec::<PackageAction>::new();

    for i in 0..action_count as isize {
        println!("Action item: {}", i);

        let ptr = unsafe { c_actions.offset(i) };
        let c_action = unsafe { &*ptr };
        
        println!("Get package key as C string");
        let package_key = unsafe { CStr::from_ptr(c_action.package_key) }.to_string_lossy();
        println!("HERP DERP: {}", &package_key);
        let package_key = AbsolutePackageKey::from_string(&package_key).unwrap();
        println!("HERP DERP?: {:?}", package_key);

        println!("Resolving packages");
        let package_record = match store.resolve_package(&package_key) {
            Some(p) => p,
            None => {
                let msg = format!("Unable to resolve package {:?}", package_key.to_string());
                eprintln!("{}", &msg);
                set_error(error,
                    ErrorCode::PackageResolveError.to_u32(),
                    &msg
                );
                return std::ptr::null()
            }
        };
        println!("Resolved package record");

        let action = PackageAction {
            package: package_record,
            action: PackageActionType::from_u8(c_action.action),
            target: if c_action.target == 0 { MacOSInstallTarget::System } else { MacOSInstallTarget::User }
        };

        println!("Finding deps");
        if action.action == PackageActionType::Install {
            let dependencies = match store.find_package_dependencies(&action.package, action.target) {
                Ok(d) => d,
                Err(_) => {
                    set_error(error,
                        ErrorCode::PackageDependencyError.to_u32(),
                        "Failed to find package dependencies"
                    );
                    return std::ptr::null();
                }
            };

            for dependency in dependencies {
                let dependency_action = PackageAction {
                    package: store.find_package(&dependency.id.to_string()).unwrap(),
                    action: PackageActionType::Install,
                    target: action.target
                };
                if let Err((code, message)) = add_package_transaction_action(dependency_action, &mut actions) {
                    set_error(error, code.to_u32(), &message);
                    return std::ptr::null();
                }
            }
        }

        println!("Found deps");
        if let Err((code, message)) = add_package_transaction_action(action, &mut actions) {
            set_error(error, code.to_u32(), &message);
            return std::ptr::null();
        }
    }

    println!("Doing the package doneness");

    let tx = PackageTransaction::new(store.clone(), actions);
    let store = Arc::into_raw(store);
    std::mem::forget(store);

    Box::into_raw(Box::from(tx))
}

fn add_package_transaction_action(
    new_action: PackageAction,
    actions: &mut Vec<PackageAction>
) -> Result<(), (ErrorCode, String)> {
    match actions.iter().find(|a| a.package.id() == new_action.package.id()) {
        Some(a) => {
            if a.action != new_action.action {
                return Err((
                    ErrorCode::PackageActionContradiction,
                    format!("The package {} has already been added but with the contradicting action", new_action.package.id().to_string()).to_string()
                ))
            }
        }
        None => actions.push(new_action),
    }
    Ok(())
}

// extern pahkat_transaction_t*
// pahkat_create_package_transaction(
//     const pahkat_client_t* _Nonnull handle,
//     const uint32_t action_count,
//     const pahkat_action_t** _Nonnull actions
// );

#[repr(C)]
#[derive(Debug)]
struct PahkatError {
    pub code: u32,
    pub message: CString
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
    // let store = unsafe { Arc::from_raw(handle) };
    let transaction = safe_handle_mut!(transaction);

    transaction.process(move |key, event| {
        progress(tx_id, CString::new(key.to_string()).unwrap().into_raw(), event.to_u32())
    });

    0
}
// extern uint32_t
// pahkat_run_package_transaction(
//     const pahkat_client_t* _Nonnull handle,
//     pahkat_transaction_t* _Nonnull transaction,
//     void (*progress)(const char* /* package_id */, uint32_t /* action */)
// );

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
                message: c_message
            }));
        }
    }
}
