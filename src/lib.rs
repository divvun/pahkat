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
