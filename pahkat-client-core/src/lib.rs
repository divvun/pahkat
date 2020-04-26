#![recursion_limit = "2048"]
#![deny(unused_must_use)]
#![deny(rust_2018_idioms)]

pub extern crate pahkat_types as types;

#[cfg(feature = "ffi")]
pub mod ffi;

pub mod config;
pub mod defaults;
pub mod package_store;
pub mod repo;
pub mod transaction;

mod cmp;
mod download;
mod ext;
mod fbs;

pub use self::config::{Config, Permission};
pub use self::download::Download;
pub use self::package_store::{DownloadEvent, InstallTarget, PackageStore};
pub use self::repo::{LoadedRepository, PackageKey};
pub use self::transaction::{PackageAction, PackageActionType, PackageStatus, PackageTransaction};

#[cfg(all(target_os = "macos", feature = "macos"))]
pub use package_store::macos::MacOSPackageStore;

#[cfg(feature = "prefix")]
pub use package_store::prefix::PrefixPackageStore;

#[cfg(all(windows, feature = "windows"))]
pub use package_store::windows::WindowsPackageStore;

pub(crate) use fbs::generated::pahkat as pahkat_fbs;
