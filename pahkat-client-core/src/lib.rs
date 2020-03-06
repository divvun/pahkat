pub extern crate pahkat_types as types;

#[cfg(feature = "ffi")]
pub mod ffi;

pub mod defaults;
pub mod package_store;
pub mod repo;
pub mod transaction;

mod cmp;
mod download;
mod ext;
mod store_config;

pub use self::download::Download;
pub use self::package_store::PackageStore;
pub use self::repo::{PackageKey, RepoRecord, Repository};
pub use self::store_config::StoreConfig;
pub use self::transaction::PackageAction;

#[cfg(all(target_os = "macos", feature = "macos"))]
pub use package_store::macos::MacOSPackageStore;

#[cfg(feature = "prefix")]
pub use package_store::prefix::PrefixPackageStore;

#[cfg(all(windows, feature = "windows"))]
pub use package_store::windows::WindowsPackageStore;

use once_cell::sync::Lazy;
use std::sync::Mutex;

static BASIC_RUNTIME: Lazy<Mutex<tokio::runtime::Runtime>> = Lazy::new(|| {
    Mutex::new(
        tokio::runtime::Builder::new()
            .basic_scheduler()
            .enable_all()
            .build()
            .expect("failed to build tokio runtime"),
    )
});

fn block_on<F: std::future::Future>(future: F) -> F::Output {
    BASIC_RUNTIME.lock().unwrap().block_on(future)
}