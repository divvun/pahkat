use std::convert::TryFrom;
use std::ffi::{CStr, OsStr};
#[cfg(unix)]
use std::os::unix::ffi::OsStrExt;
use std::path::Path;
use std::ptr::NonNull;

use cursed::prelude::{
    not_null, null, nullable_arc, throw, try_as_arc, try_as_ref, try_as_str, try_into_arc,
    try_not_null, ArcPtr, Exception, In, InOut, InRaw, Nullable, Out, OutPtr
};
use libc::{c_char, c_void};
use pahkat_types::Package;

use crate::tarball::PrefixPackageStore;

macro_rules! ok_or_throw {
    ($item:expr, $exception:expr) => {
        match $item {
            Ok(item) => nullable_arc(item),
            Err(e) => throw(&e, $exception),
        }
    };
}

#[inline]
fn cstr_to_path<'a>(c_str: &'a NonNull<c_char>) -> &'a Path {
    // Safe because pointer can never be null
    let slice = unsafe { std::ffi::CStr::from_ptr(c_str.as_ptr()) };

    let osstr = std::ffi::OsStr::from_bytes(slice.to_bytes());
    let path: &std::path::Path = osstr.as_ref();
    path
}

#[no_mangle]
pub extern "C" fn pahkat_prefix_package_store_create(
    path: In<c_char>,
    exception: OutPtr<Exception>,
) -> Nullable<ArcPtr<PrefixPackageStore>> {
    let path = try_not_null!(path.as_ptr(), &exception);
    let path: &Path = cstr_to_path(&path);
    ok_or_throw!(PrefixPackageStore::create(path), &exception)
}

#[no_mangle]
pub extern "C" fn pahkat_prefix_package_store_open(
    path: In<c_char>,
    exception: OutPtr<Exception>,
) -> Nullable<ArcPtr<PrefixPackageStore>> {
    let path = try_not_null!(path.as_ptr(), &exception);
    let path: &Path = cstr_to_path(&path);
    ok_or_throw!(PrefixPackageStore::open(path), &exception)
}

// #[no_mangle]
// extern "C" fn pahkat_prefix_package_store_resolve_package(handle: In<ArcPtr<PrefixPackageStore>>) {

// }

// #[no_mangle]
// extern "C" fn pahkat_prefix_package_store_download(...) {

// }

// #[no_mangle]
// extern "C" fn pahkat_prefix_package_store_install(...) {

// }

// #[no_mangle]
// extern "C" fn pahkat_prefix_package_store_uninstall(...) {

// }

// #[no_mangle]
// extern "C" fn pahkat_prefix_package_store_status(...) {

// }

use crate::AbsolutePackageKey;

#[no_mangle]
pub extern "C" fn pahkat_prefix_package_store_find_package_by_id(
    handle: In<ArcPtr<PrefixPackageStore>>,
    package_id: In<c_char>,
    exception: OutPtr<Exception>,
) -> Nullable<(AbsolutePackageKey, Package)> {
    let handle = unsafe { try_as_ref!(handle, &exception) };
    let package_id = try_as_str!(package_id, &exception);
    handle.find_package_by_id(package_id).into()
}

#[no_mangle]
pub extern "C" fn pahkat_print_debug_info() {
    pretty_env_logger::init();
    println!("Home directory: {:?}", &dirs::home_dir());
}

// #[no_mangle]
// extern "C" fn pahkat_prefix_package_store_find_package_dependencies(...) {

// }

// #[no_mangle]
// extern "C" fn pahkat_prefix_package_store_refresh_repos() {

// }

// #[no_mangle]
// extern "C" fn pahkat_prefix_package_store_force_refresh_repos(...) {

// }

// #[no_mangle]
// extern "C" fn pahkat_prefix_package_store_clear_cache(...) {

// }

// #[no_mangle]
// extern "C" fn pahkat_prefix_package_store_add_repo(...) {

// }

// #[no_mangle]
// extern "C" fn pahkat_prefix_package_store_remove_repo(...) {

// }

// #[no_mangle]
// extern "C" fn pahkat_prefix_package_store_update_repo(...) {

// }

// #[no_mangle]
// extern "C" fn pahkat_retain(reference: *mut c_void) {}

// #[no_mangle]
// extern "C" fn pahkat_release() {}

#[no_mangle]
pub extern "C" fn pahkat_exception_release(exception: *mut c_char) -> bool {
    match NonNull::new(exception) {
        Some(ptr) => {
            unsafe { Exception::from_raw(ptr) };
            true
        }
        None => false,
    }
}

pub type PrefixTarget = ();
pub type PrefixPackageAction = crate::transaction::PackageAction<PrefixTarget>;
pub type PrefixPackageTransaction = crate::transaction::PackageTransaction<PrefixTarget>;

#[no_mangle]
pub extern "C" fn pahkat_prefix_action_new_install(
    key: In<ArcPtr<AbsolutePackageKey>>,
    exception: OutPtr<Exception>,
) -> Nullable<ArcPtr<PrefixPackageAction>> {
    let key = try_as_arc!(key, &exception);
    // let key = try_as_ref!(key, &exception);
    nullable_arc(PrefixPackageAction::install(key.as_ref().clone(), ()))
}

#[no_mangle]
pub extern "C" fn pahkat_prefix_action_new_uninstall(
    key: In<ArcPtr<AbsolutePackageKey>>,
    exception: OutPtr<Exception>,
) -> Nullable<ArcPtr<PrefixPackageAction>> {
    let key = try_as_arc!(key, &exception);
    // let key: () = try_as_ref!(key.as_ptr().as_ptr(), &exception);
    nullable_arc(PrefixPackageAction::uninstall(key.as_ref().clone(), ()))
}

#[no_mangle]
extern "C" fn pahkat_prefix_transaction_new(
    handle: In<ArcPtr<PrefixPackageStore>>,
    actions: In<ArcPtr<cursed::vec::Vec<PrefixPackageAction>>>,
    exception: OutPtr<Exception>,
) -> Nullable<ArcPtr<PrefixPackageTransaction>> {
    let store: std::sync::Arc<PrefixPackageStore> =
        unsafe { try_into_arc!(handle.as_ptr(), &exception) };
    let actions = try_as_arc!(actions, &exception);
    let actions = actions.to_owned_vec();
    ok_or_throw!(
        PrefixPackageTransaction::new(store, actions),
        &exception
    )
}

#[no_mangle]
extern "C" fn pahkat_prefix_transaction_actions(
    handle: In<ArcPtr<PrefixPackageTransaction>>,
    exception: OutPtr<Exception>,
) -> Nullable<ArcPtr<cursed::vec::Vec<PrefixPackageAction>>> {
    let tx: std::sync::Arc<PrefixPackageTransaction> =
        unsafe { try_into_arc!(handle.as_ptr(), &exception) };
    nullable_arc(cursed::vec::Vec::from(&*tx.actions()))
}

#[no_mangle]
extern "C" fn pahkat_prefix_transaction_validate(
    handle: In<ArcPtr<PrefixPackageTransaction>>,
    exception: OutPtr<Exception>,
) -> bool {
    let tx: std::sync::Arc<PrefixPackageTransaction> =
        unsafe { try_into_arc!(handle.as_ptr(), &exception, false) };
    tx.validate()
}

#[no_mangle]
extern "C" fn pahkat_prefix_transaction_process() {}

#[no_mangle]
extern "C" fn pahkat_prefix_transaction_cancel() {}
