
#[cfg(feature = "ffi")]
pub mod ffi;

pub mod repo;
pub mod transaction;
pub mod defaults;
pub mod package_store;

mod cmp;
mod download;
mod store_config;

pub use self::download::Download;
pub use self::repo::{RepoRecord, Repository, AbsolutePackageKey};
pub use self::transaction::PackageAction;
pub use self::store_config::StoreConfig;

#[cfg(all(target_os = "macos", feature = "macos"))]
pub use package_store::macos::MacOSPackageStore;

#[cfg(feature = "prefix")]
pub use package_store::prefix::PrefixPackageStore;

#[cfg(all(windows, feature = "windows"))]
pub use package_store::windows::WindowsPackageStore;
