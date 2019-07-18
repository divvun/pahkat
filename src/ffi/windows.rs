use std::convert::TryFrom;
use std::ffi::{CStr, OsStr};
use std::path::{Path, PathBuf};
use std::ptr::NonNull;

use crate::StoreConfig;
use libc::{c_char, c_void, wchar_t};
use pahkat_types::Package;
use std::sync::RwLock;
use std::os::windows::ffi::OsStrExt;

use crate::AbsolutePackageKey;
use crate::windows::WindowsPackageStore;
use std::os::windows::ffi::OsStringExt;
use std::sync::Arc;

pub type WindowsTarget = pahkat_types::InstallTarget;
pub type WindowsPackageAction = crate::transaction::PackageAction<WindowsTarget>;
pub type WindowsPackageTransaction = crate::transaction::PackageTransaction<WindowsTarget>;
type StoreConfigPtr = *mut RwLock<StoreConfig>;

struct WindowsPathMarshaler<'a>(&'a PhantomData<u8>);

impl<'a> FromForeign for WindowsPathMarshaler<'a> {
    type In = *const wchar_t;
    type Out = PathBuf;
    type Error = Box<dyn Error>;

    fn from_foreign(c_wstr: *const wchar_t) -> Result<PathBuf, Self::Error> {
        let len = unsafe { libc::wcslen(c_wstr) };
        let slice: &[u16] = unsafe { std::slice::from_raw_parts(c_wstr, len) };
        let osstr = std::ffi::OsString::from_wide(slice);
        Ok(osstr.into())
    }
}

impl ToForeign<PathBuf> for WindowsPathMarshaler<'_> {
    type Out = *const wchar_t;
    type Error = Box<dyn Error>;

    fn to_foreign(input: PathBuf) -> Result<*const wchar_t, Self::Error> {
        let mut vec: Vec<wchar_t> = input.into_os_string().encode_wide().chain(Some(0).into_iter()).collect();
        vec.shrink_to_fit();
        let ptr = vec.as_ptr();
        std::mem::forget(vec);
        Ok(ptr)
    }

    fn drop_foreign(ptr: *const wchar_t) {
        let len = unsafe { libc::wcslen(ptr) };
        unsafe { Vec::from_raw_parts(ptr as *mut wchar_t, len, len) };
    }
}

#[no_mangle]
pub extern "C" fn pahkat_windows_enable_logging() {
    use std::io::Write;

    std::env::set_var("RUST_LOG", "pahkat_client=debug");
    env_logger::builder()
        .format(|buf, record| writeln!(buf, "{} {} {}:{} > {}", record.level(), record.target(), record.file().unwrap_or("<unknown>"), record.line().unwrap_or(0), record.args()))
        .init();
}

#[no_mangle]
pub extern "C" fn pahkat_windows_package_store_default() -> *const WindowsPackageStore {
    Arc::into_raw(Arc::new(WindowsPackageStore::default()))
}

#[no_mangle]
pub extern "C" fn pahkat_windows_package_store_new(
    path: *mut wchar_t,
    exception: *mut *mut Exception,
) -> *mut WindowsPackageStore {  //*mut WindowsPackageStore {
    log::debug!("pahkat_windows_package_store_new");
    let path = WindowsPathMarshaler::from_foreign(path).unwrap();
    log::debug!("P: {:?}", &path);
    let config = StoreConfig::new(&path);
    config.save().unwrap(); // TODO
    let store = WindowsPackageStore::new(config);
    Arc::into_raw(Arc::new(store)) as *mut _
}

#[no_mangle]
pub extern "C" fn pahkat_windows_package_store_load(
    path: *mut wchar_t,
    exception: *mut *mut Exception,
) -> *const WindowsPackageStore {  //*mut WindowsPackageStore {
    log::debug!("pahkat_windows_package_store_load");
    let path = WindowsPathMarshaler::from_foreign(path).unwrap();
    log::debug!("P: {:?}", &path);
    let config = match StoreConfig::load(&path, true) {
        Ok(v) => v,
        Err(err) => {
            throw(err, exception);
            return std::ptr::null();
        }
    };
    let store = WindowsPackageStore::new(config);
    Arc::into_raw(Arc::new(store)) as *mut _
}


trait ToForeign<In>: Sized {
    // type In;
    type Out;
    type Error;
    fn to_foreign(_: In) -> Result<Self::Out, Self::Error>;
    fn drop_foreign(_: Self::Out) {}
}

trait FromForeign: Sized {
    type In;
    type Out;
    type Error;
    fn from_foreign(_: Self::In) -> Result<Self::Out, Self::Error>;
    fn drop_local(_: Self::Out) {}
}

use std::error::Error;
use crate::transaction::{PackageStatus, PackageStatusError};
use std::borrow::Cow;

struct StrMarshaler<'a>(&'a PhantomData<u8>);

impl<'a> FromForeign for StrMarshaler<'a> {
    type In = *const c_char;
    type Out = Cow<'a, str>;
    type Error = Box<dyn Error>;

    fn from_foreign(key: *const c_char) -> Result<Cow<'a, str>, Self::Error> {
        Ok(unsafe { CStr::from_ptr(key) }.to_string_lossy())
    }
}

impl<'a> ToForeign<&'a str> for StrMarshaler<'a> {
    type Out = *const c_char;
    type Error = Box<dyn Error>;

    fn to_foreign(input: &'a str) -> Result<*const c_char, Self::Error> {
        let c_str = CString::new(input)?;
        Ok(c_str.into_raw())
    }

    fn drop_foreign(ptr: *const c_char) {
        unsafe { CString::from_raw(ptr as *mut _) };
    }
}

struct PackageKeyMarshaler;

impl FromForeign for PackageKeyMarshaler {
    type In = *const c_char;
    type Out = AbsolutePackageKey;
    type Error = Box<dyn Error>;
    
    fn from_foreign(key: *const c_char) -> Result<AbsolutePackageKey, Self::Error> {
        let key = unsafe { CStr::from_ptr(key) }.to_string_lossy();
        AbsolutePackageKey::from_string(&*key)
    }
}

impl ToForeign<AbsolutePackageKey> for PackageKeyMarshaler {
    type Out = *const c_char;
    type Error = Box<dyn Error>;

    fn to_foreign(input: AbsolutePackageKey) -> Result<*const c_char, Self::Error> {
        let key = input.to_string();
        log::debug!("PKG KEY: {}", &key);
        let c_str = CString::new(key)?;
        Ok(c_str.into_raw())
    }

    fn drop_foreign(ptr: *const c_char) {
        unsafe { CString::from_raw(ptr as *mut _) };
    }
}

struct PackageStatusMarshaler;

impl PackageStatusMarshaler {
    fn status_to_int(status: PackageStatus) -> i8 {
        match status {
            PackageStatus::NotInstalled => 0,
            PackageStatus::UpToDate => 1,
            PackageStatus::RequiresUpdate => 2,
            PackageStatus::Skipped => 3,
        }
    }

    fn error_to_int(error: PackageStatusError) -> i8 {
        use PackageStatusError::*;

        match error {
            NoPackage => -1,
            NoInstaller => -2,
            WrongInstallerType => -3,
            ParsingVersion => -4,
            InvalidInstallPath => -5,
            InvalidMetadata => -6
        }
    }
}

impl ToForeign<Result<PackageStatus, PackageStatusError>> for PackageStatusMarshaler {
    // type In = ;
    type Out = i8;
    type Error = Box<dyn Error>;
    
    fn to_foreign(status: Result<PackageStatus, PackageStatusError>) -> Result<i8, Self::Error> {
        let result = match status {
            Ok(status) => PackageStatusMarshaler::status_to_int(status),
            Err(error) => PackageStatusMarshaler::error_to_int(error)
        };

        Ok(result)
    }
}

use crate::transaction::PackageStore;

#[no_mangle]
extern "C" fn pahkat_windows_package_store_status(
    handle: *mut WindowsPackageStore,
    package_key: *const c_char,
    mut is_system: *mut bool,
    exception: *mut *mut Exception, // out
) -> i8 {
    let store = unsafe { &mut *handle };
    let package_key = PackageKeyMarshaler::from_foreign(package_key).unwrap();
    let is_system = unsafe { &mut *is_system };

    fn package_store_status(
        store: &WindowsPackageStore,
        package_key: &AbsolutePackageKey,
        is_system: &mut bool
    ) -> Result<Result<PackageStatus, PackageStatusError>, Box<dyn Error>> {
        let status = store.status(package_key, &WindowsTarget::User);
        match status {
            Ok(result) => {
                match result {
                    PackageStatus::NotInstalled => {},
                    _ => {
                        *is_system = false;
                        return Ok(status)
                    }
                }
            }
            Err(_) => {}
        }

        *is_system = true;
        Ok(store.status(package_key, &WindowsTarget::System))
    }

    match package_store_status(&store, &package_key, is_system) {
        Ok(result) => {
            match PackageStatusMarshaler::to_foreign(result) {
                Ok(foreign) => foreign,
                Err(err) => {
                    throw(err, exception);
                    i8::default()
                }
            }
        },
        Err(err) => {
            throw(err, exception);
            i8::default()
        }
    }
}


#[no_mangle]
extern "C" fn pahkat_windows_package_store_download(
    handle: *mut WindowsPackageStore,
    package_key: *const c_char,
    progress: extern "C" fn(*const AbsolutePackageKey, u64, u64),
    exception: *mut *mut Exception,
) -> *const wchar_t {
    let store = unsafe { &mut *handle };
    let package_key = PackageKeyMarshaler::from_foreign(package_key).unwrap();

    fn package_store_download(
        store: &WindowsPackageStore,
        package_key: &AbsolutePackageKey,
        progress: extern "C" fn(*const AbsolutePackageKey, u64, u64),
    ) -> Result<PathBuf, Box<dyn Error>> {
        let package_key1 = package_key.to_owned();
        store.download(&package_key, Box::new(move |cur, max| {
            progress(&package_key1 as *const _, cur, max);
        })).map_err(|e| Box::new(e) as Box<dyn Error>)
    }

    match package_store_download(&store, &package_key, progress) {
        Ok(result) => {
            match WindowsPathMarshaler::to_foreign(result) {
                Ok(v) => v,
                Err(err) => {
                    throw(err, exception);
                    return std::ptr::null();
                }
            }
        },
        Err(err) => {
            throw(err, exception);
            std::ptr::null()
        }
    }
}

#[no_mangle]
extern "C" fn pahkat_windows_package_store_clear_cache(
    handle: *mut WindowsPackageStore,
    exception: *mut *mut Exception,    
) {
    let store = unsafe { &mut *handle };
    store.clear_cache();
}

#[no_mangle]
extern "C" fn pahkat_windows_package_store_refresh_repos(
    handle: *mut WindowsPackageStore,
    exception: *mut *mut Exception,    
) {
    let store = unsafe { &mut *handle };
    store.refresh_repos();
}

#[no_mangle]
extern "C" fn pahkat_windows_package_store_force_refresh_repos(
    handle: *mut WindowsPackageStore,
    exception: *mut *mut Exception,    
) {
    let store = unsafe { &mut *handle };
    store.force_refresh_repos();
}

use std::marker::PhantomData;
use std::ffi::CString;
use serde::{Deserialize, Serialize};
use serde::de::DeserializeOwned;

struct JsonMarshaler<'a, T>(&'a PhantomData<T>);

impl<'a, T> ToForeign<&'a T> for JsonMarshaler<'a, T> where T: Serialize {
    // type In = &'a T;
    type Out = *const c_char;
    type Error = Box<dyn Error>;
    
    fn to_foreign(input: &'a T) -> Result<*const c_char, Self::Error> {
        let vec = serde_json::to_vec(input)?;
        let c_str = CString::new(vec)?;
        log::debug!("JSON MARSHAL: {:?}", &c_str);
        Ok(c_str.into_raw())
    }

    fn drop_foreign(ptr: *const c_char) {
        unsafe { CString::from_raw(ptr as *mut _) };
    }
}

impl<'a, T> FromForeign for JsonMarshaler<'a, T> where T: DeserializeOwned {
    type In = *const c_char;
    type Out = T;
    type Error = Box<dyn Error>;
    
    fn from_foreign(ptr: *const c_char) -> Result<T, Self::Error> {
        let s = unsafe { CStr::from_ptr(ptr) }.to_string_lossy();
        log::debug!("JSON: {}", s);
        serde_json::from_str(&s).map_err(|e| Box::new(e) as _)
    }

    // fn drop_local(ptr: *const c_char) {
    //     unsafe { CString::from_raw(ptr as *mut _) };
    // }
}

#[no_mangle]
extern "C" fn pahkat_windows_package_store_repo_indexes(
    handle: *mut WindowsPackageStore,
    exception: *mut *mut Exception,
) -> *const c_char {
    let store = unsafe { &mut *handle };
    let rwlock = store.repos();
    let guard = rwlock.read().unwrap();
    let indexes = guard.values().collect::<Vec<&_>>();
    match JsonMarshaler::to_foreign(&indexes) {
        Ok(v) => v,
        Err(err) => {
            throw(err, exception);
            std::ptr::null()
        }
    }
}

// #[no_mangle]
// extern "C" fn pahkat_windows_package_store_install(...) {

// }

// #[no_mangle]
// extern "C" fn pahkat_windows_package_store_uninstall(...) {

// }


// pub extern "C" fn pahkat_windows_package_store_read_only???


struct ArcMarshaler<'a, T>(&'a PhantomData<T>);

// impl<'a, T> ToForeign<&'a Arc<T>> for ArcMarshaler<'a, T> {
//     // type In = &'a Arc<T>;
//     type Out = *const T;
//     type Error = Box<dyn Error>;
    
//     fn to_foreign(input: &'a Arc<T>) -> Result<*const T, Self::Error> {
//         let arc = Arc::clone(input);
//         Ok(Arc::into_raw(arc))
//     }

//     fn drop_foreign(ptr: *const T) {
//         unsafe { Arc::from_raw(ptr) };
//     }
// }

// impl<T> ToForeign<T> for ArcMarshaler<'_, T> {
//     type Out = *const T;
//     type Error = Box<dyn Error>;
    
//     fn to_foreign(input: T) -> Result<*const T, Self::Error> {
//         let arc = Arc::new(input);
//         Ok(Arc::into_raw(arc))
//     }

//     fn drop_foreign(ptr: *const T) {
//         unsafe { Arc::from_raw(ptr) };
//     }
// }

impl<T> ToForeign<Arc<T>> for ArcMarshaler<'_, T> {
    // type In = &'a Arc<T>;
    type Out = *const T;
    type Error = Box<dyn Error>;
    
    fn to_foreign(input: Arc<T>) -> Result<*const T, Self::Error> {
        Ok(Arc::into_raw(input))
    }

    fn drop_foreign(ptr: *const T) {
        unsafe { Arc::from_raw(ptr) };
    }
}

impl<'a, T> FromForeign for ArcMarshaler<'a, T> {
    type In = *const T;
    type Out = Arc<T>;
    type Error = Box<dyn Error>;
    
    fn from_foreign(input: *const T) -> Result<Arc<T>, Self::Error> {
        let arc = unsafe { Arc::from_raw(input) };
        let ret = Arc::clone(&arc);
        std::mem::forget(arc);
        Ok(ret)
    }
}

#[no_mangle]
pub extern "C" fn pahkat_exception_release(c_str: *mut c_char) {
    unsafe { CString::from_raw(c_str) };
}

#[no_mangle]
pub extern "C" fn pahkat_windows_package_store_config(
    handle: *mut WindowsPackageStore,
    exception: *mut *mut Exception,
) -> *const RwLock<StoreConfig> {
    let store = unsafe { &mut *handle };
    
    fn package_store_config(store: &WindowsPackageStore) -> Result<Arc<RwLock<StoreConfig>>, Box<dyn Error>> {
        Ok(store.config())
    }

    let result = match package_store_config(&store) {
        Ok(v) => v,
        Err(err) => {
            throw(err, exception);
            return std::ptr::null_mut();
        }
    };

    let wat = ArcMarshaler::to_foreign(result);
    match wat {
        Ok(v) => v,
        Err(err) => {
            throw(err, exception);
            std::ptr::null()
        }
    }
}

#[no_mangle]
pub extern "C" fn pahkat_store_config_set_ui_value(
    handle: StoreConfigPtr,
    key: *const c_char,
    value: *const c_char,
    exception: *mut *mut Exception,
) {
    let handle: &mut RwLock<StoreConfig> = unsafe { &mut *handle };
    let key = StrMarshaler::from_foreign(key).unwrap();
    let value: Option<String> = match value.is_null() {
        true => None,
        false => {
            Some(StrMarshaler::from_foreign(value).unwrap().to_string())
        }
    };
    // log::debug!("Key: {:?}, Value: {:?}", &key, &value);
    // log::debug!("Getting guard");
    {
        let guard = handle.write().unwrap();
        // log::debug!("Got guard!");
        guard.set_ui_value(&key, value).expect("set_ui_value")
    }
    // ret
}

#[no_mangle]
pub extern "C" fn pahkat_store_config_ui_value(
    handle: StoreConfigPtr,
    key: *const c_char,
    exception: *mut *mut Exception,
) -> *const c_char {
    let handle: &mut RwLock<StoreConfig> = unsafe { &mut *handle };
    let key = StrMarshaler::from_foreign(key).unwrap();
    let guard = handle.read().unwrap();

    match guard.ui_value(&key) {
        Some(v) => match StrMarshaler::to_foreign(&v) {
            Ok(v) => v,
            Err(_) => std::ptr::null(),
        },
        None => std::ptr::null(),
    }
}

// #[no_mangle]
// pub extern "C" fn pahkat_store_config_remove_skipped_package(
//     handle: StoreConfigPtr,
//     key: *const c_char,
//     exception: *mut *mut Exception,
// ) {    
//     let handle: &mut RwLock<StoreConfig> = unsafe { &mut *handle };
//     let guard = handle.write().unwrap();
// }

// #[no_mangle]
// pub extern "C" fn pahkat_store_config_add_skipped_package(
//     handle: StoreConfigPtr,
//     key: *const c_char,
//     version: *const c_char,
//     exception: *mut *mut Exception,
// ) {
//     let handle: &mut RwLock<StoreConfig> = unsafe { &mut *handle };
//     let guard = handle.write().unwrap();

//     guard.add_skipped_package
// }

#[no_mangle]
pub extern "C" fn pahkat_store_config_skipped_package(
    handle: StoreConfigPtr,
    key: *const c_char,
    exception: *mut *mut Exception,
) -> *const c_char {
    let config: Arc<_> = ArcMarshaler::from_foreign(handle).unwrap();
    let guard = config.read().unwrap();
    let key = PackageKeyMarshaler::from_foreign(key).unwrap();

    match guard.skipped_package(&key) {
        Some(v) => StrMarshaler::to_foreign(&v).unwrap(),
        None => std::ptr::null()
    }
}

#[no_mangle]
pub extern "C" fn pahkat_store_config_repos(
    handle: StoreConfigPtr,
    exception: *mut *mut Exception,
) -> *const c_char {
    let handle: &mut RwLock<StoreConfig> = unsafe { &mut *handle };
    let guard = handle.read().unwrap();

    JsonMarshaler::to_foreign(&guard.repos()).unwrap()
}

use crate::RepoRecord;

#[no_mangle]
pub extern "C" fn pahkat_store_config_set_repos(
    handle: StoreConfigPtr,
    repos: *const c_char,
    exception: *mut *mut Exception,
) {
    let handle: &mut RwLock<StoreConfig> = unsafe { &mut *handle };
    let guard = handle.write().unwrap();

    let repos: Vec<RepoRecord> = JsonMarshaler::from_foreign(repos).unwrap();
    guard.set_repos(repos);
}

#[no_mangle]
pub extern "C" fn pahkat_windows_action_to_json(
    action: *mut WindowsPackageAction,
    exception: *mut *mut Exception,
) -> *const c_char {
    let action = unsafe { &mut *action };
    match JsonMarshaler::to_foreign(action) {
        Ok(v) => v,
        Err(err) => {
            throw(err, exception);
            std::ptr::null()
        }
    }
}

#[no_mangle]
pub extern "C" fn pahkat_windows_action_from_json(
    action: *mut c_char,
    exception: *mut *mut Exception,
) -> *const WindowsPackageAction {
    let result: WindowsPackageAction = match JsonMarshaler::from_foreign(action) {
        Ok(v) => v,
        Err(err) => {
            throw(err, exception);
            return std::ptr::null();
        }
    };

    match ArcMarshaler::to_foreign(Arc::new(result)) {
        Ok(v) => v,
        Err(err) => {
            throw(err, exception);
            std::ptr::null()
        }
    }
}

#[no_mangle]
extern "C" fn pahkat_windows_transaction_new(
    handle: *mut WindowsPackageStore,
    actions: *const c_char,
    exception: *mut *mut Exception,
) -> *const WindowsPackageTransaction {
    let store: Arc<WindowsPackageStore> = ArcMarshaler::from_foreign(handle).unwrap();
    let actions: Vec<WindowsPackageAction> = JsonMarshaler::from_foreign(actions).unwrap();

    fn transaction_new(
        store: Arc<WindowsPackageStore>,
        actions: Vec<WindowsPackageAction>
    ) -> Result<WindowsPackageTransaction, Box<dyn Error>> {
        WindowsPackageTransaction::new(store as Arc<_>, actions)
            .map_err(|e| Box::new(e) as _)
    }
    
    let result = match transaction_new(store, actions) {
        Ok(v) => v,
        Err(err) => {
            throw(err, exception);
            return std::ptr::null();
        }
    };

    match ArcMarshaler::to_foreign(Arc::new(result)) {
        Ok(v) => v,
        Err(err) => {
            throw(err, exception);
            std::ptr::null()
        }
    }
}

#[no_mangle]
extern "C" fn pahkat_windows_transaction_actions(
    handle: *mut WindowsPackageTransaction,
    exception: *mut *mut Exception,
) -> *const c_char {
    let tx = unsafe { &mut *handle };
    JsonMarshaler::to_foreign(&*tx.actions()).unwrap()
}

// #[no_mangle]
// extern "C" fn pahkat_windows_transaction_validate(
//     handle: *mut WindowsPackageTransaction,
//     exception: *mut *mut Exception,
// ) -> bool {
//     let tx: std::sync::Arc<WindowsPackageTransaction> =
//         unsafe { try_into_arc!(handle.as_ptr(), exception, false) };
//     tx.validate()
// }

#[no_mangle]
extern "C" fn pahkat_windows_transaction_process(
    handle: *mut WindowsPackageTransaction,
    progress_callback: extern "C" fn (u32, *const libc::c_char, u32),
    tag: u32,
    exception: *mut *mut Exception,
) {
    let tx = unsafe { &mut *handle };
    
    tx.process(move |key, event| {
        let k = PackageKeyMarshaler::to_foreign(key).unwrap();
        progress_callback(tag, k, event.to_u32());
        PackageKeyMarshaler::drop_foreign(k);
    })
}

// #[no_mangle]
// extern "C" fn pahkat_windows_transaction_cancel(
//     handle: *mut WindowsPackageTransaction,
//     exception: *mut *mut Exception,
// ) -> bool {
//     let tx = unsafe { try_as_ref!(handle, exception, false) };
//     tx.cancel()
// }

// #[no_mangle]
// extern "C" fn pahkat_box_release(
//     handle: *mut libc::c_void,
//     exception: *mut *mut Exception,
// ) {
//     // TODO: exception
//     unsafe { Box::from_raw(handle) };
// }

// #[no_mangle]
// extern "C" fn pahkat_arc_release(
//     handle: *mut libc::c_void,
//     exception: *mut *mut Exception,
// ) {
//     // TODO: exception
//     unsafe { Arc::from_raw(handle) };
// }

// use libc::c_char;
// use semver::Version;
// use serde_json::json;
// use std::ffi::{CStr, CString};
// use std::ptr::null;

// // #[no_mangle]
// // extern fn pahkat_download_package(handle: *const crate::windows::WindowsPackageStore, package_id: *const c_char, progress: extern fn(u64, u64), error: *mut u32) {

// // }

// // TODO catch unwind!!

// use crate::windows::PackageAction;
// use crate::windows::PackageTransaction;
// use crate::windows::WindowsPackageStore;
// use crate::AbsolutePackageKey;
// use crate::PackageActionType;
// use crate::PackageStatus;
// use crate::PackageTransactionError;
// use crate::RepoRecord;
// use crate::StoreConfig;
// use pahkat_types::*;
// use std::sync::Arc;

// #[repr(C)]
// struct Repo {
//     url: *const c_char,
//     channel: *const c_char,
// }

// #[no_mangle]
// extern "C" fn pahkat_error_free(error: *mut PahkatError) {
//     log::debug!("pahkat_error_free");
//     unsafe { Box::from_raw(error) };
// }

// #[no_mangle]
// extern "C" fn pahkat_config_path(handle: *const WindowsPackageStore) -> *const c_char {
//     let store = safe_handle!(handle);
//     let c_str = CString::new(&*store.config().config_path().to_string_lossy()).unwrap();
//     CString::into_raw(c_str)
// }

// #[no_mangle]
// extern "C" fn pahkat_config_repos(handle: *const WindowsPackageStore) -> *const c_char {
//     let store = safe_handle!(handle);
//     let it = serde_json::to_string(&store.config().repos()).unwrap();
//     CString::new(it).unwrap().into_raw()
// }

// #[no_mangle]
// extern "C" fn pahkat_config_set_repos(handle: *const WindowsPackageStore, repos: *const c_char) {
//     let store = safe_handle!(handle);
//     let repos = unsafe { CStr::from_ptr(repos).to_string_lossy() };
//     let repos: Vec<RepoRecord> = serde_json::from_str(&repos).unwrap();
//     store.config().set_repos(repos);
// }

// #[no_mangle]
// extern "C" fn pahkat_client_free(handle: *const WindowsPackageStore) {
//     if handle.is_null() {
//         return;
//     }

//     unsafe {
//         Arc::from_raw(safe_handle!(handle));
//     }
// }

// #[no_mangle]
// extern "C" fn pahkat_str_free(handle: *mut c_char) {
//     if handle.is_null() {
//         return;
//     }

//     unsafe {
//         CString::from_raw(safe_handle_mut!(handle));
//     }
// }

// #[no_mangle]
// extern "C" fn pahkat_repos_json(handle: *const WindowsPackageStore) -> *const c_char {
//     let store = safe_handle!(handle);

//     let repos = store.repos_json();
//     let s = CString::new(&*repos).unwrap().into_raw();

//     s
// }

// struct DownloadPackageKey(*const c_char);
// unsafe impl Send for DownloadPackageKey {}

// #[no_mangle]
// extern "C" fn pahkat_download_package(
//     handle: *const WindowsPackageStore,
//     raw_package_key: *const c_char,
//     target: u8,
//     progress: extern "C" fn(*const c_char, u64, u64) -> (),
//     error: *mut *const PahkatError,
// ) -> u32 {
//     log::debug!("pahkat_download_package");
//     let store = safe_handle!(handle);

//     if raw_package_key.is_null() {
//         let code = ErrorCode::PackageKeyError.to_u32();
//         set_error(error, code, "Package key must not be null");
//         return code;
//     }

//     let package_key = unsafe { CStr::from_ptr(raw_package_key) }.to_string_lossy();
//     let package_key = AbsolutePackageKey::from_string(&package_key).unwrap();
//     let package = match store.resolve_package(&package_key) {
//         Some(v) => v,
//         None => {
//             log::error!("Resolve package error");
//             let code = ErrorCode::PackageResolveError.to_u32();
//             set_error(
//                 error,
//                 code,
//                 &format!("Unable to resolve package {:?}", package_key.to_string()),
//             );
//             return code;
//         }
//     };

//     let download_package_key = DownloadPackageKey(raw_package_key);

//     match store.download(&package_key, move |cur, max| {
//         progress(download_package_key.0, cur, max);
//     }) {
//         Ok(_) => 0,
//         Err(e) => {
//             let code = ErrorCode::PackageDownloadError.to_u32();
//             set_error(
//                 error,
//                 code,
//                 &format!("Unable to download package {:?}", package_key.to_string()),
//             );
//             code
//         }
//     }
// }

// #[no_mangle]
// extern "C" fn pahkat_status(
//     handle: *const WindowsPackageStore,
//     package_key: *const c_char,
//     error: *mut u32,
// ) -> *const c_char {
//     // This one is nullable if there's an error.
//     let store = safe_handle!(handle);

//     if error.is_null() {
//         panic!("error must not be null");
//     }

//     unsafe {
//         *error = 0;
//     }

//     fn make_json(status: PackageStatus, target: InstallTarget) -> *const c_char {
//         let map = json!({
//             "status": status,
//             "target": target
//         })
//         .to_string();

//         CString::new(map).unwrap().into_raw()
//     }

//     if package_key.is_null() {
//         unsafe {
//             *error = 1;
//         }
//         return null();
//     }

//     let package_key = unsafe { CStr::from_ptr(package_key) }.to_string_lossy();
//     let package_key = AbsolutePackageKey::from_string(&package_key).unwrap();

//     // TODO use package key
//     // let package = match store.resolve_package(&package_key) {
//     //     Some(v) => v,
//     //     None => {
//     //         unsafe { *error = 4; }
//     //         return null();
//     //     }
//     // };

//     let pkg_status = match store.status(&package_key, InstallTarget::System) {
//         Ok(v) => v,
//         Err(e) => {
//             unsafe {
//                 *error = 10;
//             }
//             return make_json(PackageStatus::NotInstalled, InstallTarget::System);
//         }
//     };

//     match pkg_status {
//         PackageStatus::NotInstalled => {}
//         _ => {
//             return make_json(pkg_status, InstallTarget::System);
//         }
//     };

//     let pkg_status = match store.status(&package_key, InstallTarget::User) {
//         Ok(v) => v,
//         Err(e) => {
//             unsafe {
//                 *error = 10;
//             }
//             return make_json(PackageStatus::NotInstalled, InstallTarget::System);
//         }
//     };

//     make_json(pkg_status, InstallTarget::User)
// }

// #[no_mangle]
// extern "C" fn pahkat_create_action(
//     action: u8,
//     target: u8,
//     package_key: *const c_char,
// ) -> *mut PackageAction {
//     Box::into_raw(Box::new(PackageAction {
//         id: AbsolutePackageKey::from_string(
//             &*unsafe { CStr::from_ptr(package_key) }.to_string_lossy(),
//         )
//         .unwrap(),
//         action: PackageActionType::from_u8(action),
//         target: if target == 0 {
//             InstallTarget::System
//         } else {
//             InstallTarget::User
//         },
//     }))
// }

// #[no_mangle]
// extern "C" fn pahkat_create_package_transaction<'a>(
//     handle: *const WindowsPackageStore,
//     action_count: u32,
//     c_actions: *const *const PackageAction,
//     error: *mut *const PahkatError,
// ) -> *const PackageTransaction {
//     let store = unsafe { Arc::from_raw(handle) };
//     let mut actions = Vec::<PackageAction>::new();

//     for i in 0..action_count as isize {
//         let ptr = unsafe { *c_actions.offset(i) };
//         let action = unsafe { &*ptr }.to_owned();
//         actions.push(action);
//     }

//     let tx = match PackageTransaction::new(store.clone(), actions) {
//         Ok(v) => {
//             let store = Arc::into_raw(store);
//             std::mem::forget(store);
//             v
//         }
//         Err(e) => {
//             let c_error = match e {
//                 PackageTransactionError::NoPackage(id) => PahkatError {
//                     code: 1,
//                     message: CString::new(&*format!("No package with id: {}", id))
//                         .unwrap()
//                         .into_raw(),
//                 },
//                 PackageTransactionError::Deps(dep_error) => PahkatError {
//                     code: 2,
//                     message: CString::new(&*format!("{:?}", dep_error))
//                         .unwrap()
//                         .into_raw(),
//                 },
//                 PackageTransactionError::ActionContradiction(id) => PahkatError {
//                     code: 3,
//                     message: CString::new(&*format!("Package contradiction for: {}", id))
//                         .unwrap()
//                         .into_raw(),
//                 },
//             };
//             unsafe { *error = Box::into_raw(Box::new(c_error)) };
//             let store = Arc::into_raw(store);
//             std::mem::forget(store);
//             return std::ptr::null();
//         }
//     };

//     Box::into_raw(Box::from(tx))
// }

// #[repr(C)]
// #[derive(Debug)]
// struct PahkatError {
//     pub code: u32,
//     pub message: *const c_char,
// }

// impl Drop for PahkatError {
//     fn drop(&mut self) {
//         unsafe { CString::from_raw(self.message as *mut _) };
//     }
// }

// #[no_mangle]
// extern "C" fn pahkat_validate_package_transaction(
//     handle: *const WindowsPackageStore,
//     transaction: *const PackageTransaction,
//     error: *mut *const PahkatError,
// ) -> u32 {
//     0
// }

// #[no_mangle]
// extern "C" fn pahkat_run_package_transaction(
//     handle: *const WindowsPackageStore,
//     transaction: *mut PackageTransaction,
//     tx_id: u32,
//     progress: extern "C" fn(u32, *const c_char, u32),
//     error: *mut *const PahkatError,
// ) -> u32 {
//     log::debug!("pahkat_run_package_transaction");
//     let transaction = safe_handle_mut!(transaction);

//     // TODO: package transaction should also return index of package and total package numbers...
//     transaction.process(move |key, event| {
//         progress(
//             tx_id,
//             CString::new(key.to_string()).unwrap().into_raw(),
//             event.to_u32(),
//         )
//     });

//     0
// }

// #[no_mangle]
// extern "C" fn pahkat_package_transaction_actions(
//     handle: *const WindowsPackageStore,
//     transaction: *const PackageTransaction,
//     error: *mut *const PahkatError,
// ) -> *const c_char {
//     let transaction = safe_handle!(transaction);

//     let json = serde_json::to_string(&*transaction.actions()).expect("serialization issue");
//     CString::new(json).unwrap().into_raw()
// }

// #[no_mangle]
// extern "C" fn pahkat_package_install(
//     handle: *const WindowsPackageStore,
//     raw_package_key: *const c_char,
//     target: u8,
// ) {
//     let store = safe_handle!(handle);
//     let package_key = unsafe { CStr::from_ptr(raw_package_key) }.to_string_lossy();
//     let package_key = AbsolutePackageKey::from_string(&package_key).unwrap();
//     store.install(&package_key, InstallTarget::System);
// }

// #[no_mangle]
// extern "C" fn pahkat_package_path(
//     handle: *const WindowsPackageStore,
//     raw_package_key: *const c_char,
// ) -> *const c_char {
//     let store = safe_handle!(handle);
//     let package_key = unsafe { CStr::from_ptr(raw_package_key) }.to_string_lossy();
//     let package_key = AbsolutePackageKey::from_string(&package_key).unwrap();
//     match store.package_path(&package_key) {
//         Some(v) => {
//             let p = v.to_string_lossy();
//             CString::new(&*p).unwrap().into_raw()
//         }
//         None => std::ptr::null(),
//     }
// }

// #[no_mangle]
// extern "C" fn pahkat_semver_is_valid(version_str: *const c_char) -> u8 {
//     let version_string = unsafe { CStr::from_ptr(version_str) }.to_string_lossy();

//     match Version::parse(&version_string) {
//         Ok(version) => 1,
//         _ => {
//             log::error!(
//                 "pahkat_semver_is_valid: failed to parse version string: {}",
//                 &version_string
//             );
//             0
//         }
//     }
// }

// #[no_mangle]
// extern "C" fn pahkat_semver_compare(lhs: *const c_char, rhs: *const c_char) -> i32 {
//     let lhs_string = unsafe { CStr::from_ptr(lhs) }.to_string_lossy();
//     let rhs_string = unsafe { CStr::from_ptr(rhs) }.to_string_lossy();

//     let lhs_version = match Version::parse(&lhs_string) {
//         Ok(version) => version,
//         _ => {
//             log::error!("pahkat_semver_compare: lhs is not a valid semver");
//             return 0;
//         }
//     };

//     let rhs_version = match Version::parse(&rhs_string) {
//         Ok(version) => version,
//         _ => {
//             log::error!("pahkat_semver_compare: rhs is not a valid semver");
//             return 0;
//         }
//     };

//     if lhs_version < rhs_version {
//         -1
//     } else if lhs_version == rhs_version {
//         0
//     } else {
//         1
//     }
// }

// enum ErrorCode {
//     None,
//     PackageDownloadError,
//     PackageDependencyError,
//     PackageActionContradiction,
//     PackageResolveError,
//     PackageKeyError,
// }

// impl ErrorCode {
//     fn to_u32(&self) -> u32 {
//         match self {
//             ErrorCode::None => 0,
//             ErrorCode::PackageDownloadError => 1,
//             ErrorCode::PackageDependencyError => 2,
//             ErrorCode::PackageActionContradiction => 3,
//             ErrorCode::PackageResolveError => 4,
//             ErrorCode::PackageKeyError => 5,
//         }
//     }
// }

// fn set_error(error: *mut *const PahkatError, code: u32, message: &str) {
//     let c_message = match CString::new(message) {
//         Ok(s) => s,
//         Err(_) => CString::new("Failed to create CString representation").unwrap(),
//     };

//     unsafe {
//         if error.is_null() {
//             log::error!("{}", message);
//         } else {
//             *error = Box::into_raw(Box::new(PahkatError {
//                 code,
//                 message: c_message.into_raw(),
//             }));
//         }
//     }
// }
/// A newtype over a raw, owned CString for providing errors over the FFI.
#[repr(transparent)]
pub struct Exception(NonNull<c_char>);

impl Exception {
    pub unsafe fn from_raw(ptr: NonNull<c_char>) -> Exception {
        Exception(ptr)
    }

    pub fn into_c_string(self) -> CString {
        let ret = unsafe { CString::from_raw(self.0.as_ptr() as *mut _) };
        std::mem::forget(self);
        ret
    }

    pub fn as_ptr(&self) -> *const c_char {
        self.0.as_ptr()
    }

    pub fn into_raw(self) -> *mut Exception {
        let ret = self.as_ptr() as *mut _;
        std::mem::forget(self);
        ret
    }
}

impl Drop for Exception {
    fn drop(&mut self) {
        // log::debug!("EXCEPTION DROPPED: {:?}", self.0.as_ptr());
        // unsafe { log::debug!("was: {:?}", CString::from_raw(self.0.as_ptr() as *mut _)) };
    }
}

impl TryFrom<&str> for Exception {
    type Error = std::ffi::NulError;

    fn try_from(string: &str) -> Result<Exception, Self::Error> {
        let c_str = CString::new(string)?;
        Ok(Exception(NonNull::new(c_str.into_raw()).unwrap()))
    }
}

impl From<Exception> for CString {
    fn from(exception: Exception) -> CString {
        exception.into_c_string()
    }
}

#[inline]
pub fn throw_message<S: AsRef<str>>(
    msg: S,
    exception: *mut *mut Exception,
) {
    if !exception.is_null() {
        let msg = Exception::try_from(msg.as_ref()).unwrap();
        unsafe { *exception = msg.into_raw() };
    }
}

#[inline]
pub fn throw(e: impl std::fmt::Display, exception: *mut *mut Exception) {
    if !exception.is_null() {
        let msg = Exception::try_from(&*format!("{}", e)).unwrap();
        unsafe { *exception = msg.into_raw() };
    }
}
