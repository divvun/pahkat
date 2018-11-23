use libc::c_char;
use std::ffi::CString;

// #[no_mangle]
// extern fn pahkat_download_package(handle: *const crate::macos::MacOSPackageStore, package_id: *const c_char, progress: extern fn(u64, u64), error: *mut u32) {

// }

use crate::macos::MacOSPackageStore;
use crate::StoreConfig;
use crate::repo::Repository;

#[repr(C)]
struct Repo {
    url: *const c_char,
    channel: *const c_char
}

#[no_mangle]
extern fn pahkat_client_new() -> *const MacOSPackageStore {
    let config = StoreConfig::load_or_default();
    let repos = config.repos()
        .iter()
        .map(|record| Repository::from_url(&record.url).unwrap())
        .collect::<Vec<_>>();

    let store = MacOSPackageStore::new(config);

    Box::into_raw(Box::new(store))
}

#[no_mangle]
extern fn pahkat_list_repos(handle: *const MacOSPackageStore, repos: *mut *const Repo) -> u32 {
    let store = unsafe { &*handle };

    let mut c_repos = store.config().repos().iter().map(|r| Repo {
        url: CString::new(&*r.url).unwrap().into_raw(),
        channel: CString::new(&*r.channel).unwrap().into_raw()
    }).collect::<Vec<_>>();

    c_repos.shrink_to_fit();
    let len = c_repos.len() as u32;

    unsafe {
        *repos = c_repos.as_ptr();
        std::mem::forget(c_repos);
    }

    len
}

#[no_mangle]
extern fn pahkat_repo_json(handle: *const MacOSPackageStore, repo: *mut *const Repo, json: *mut *const c_char) -> u32 {
    let store = unsafe { &*handle };

    // Find repo in store, then JSONify it

    //len
    unimplemented!()
}
