#[cfg(all(target_os = "macos", feature = "macos"))]
pub mod macos;

#[cfg(all(windows, feature = "windows"))]
pub mod windows;

#[cfg(feature = "prefix")]
pub mod prefix;

mod log;
mod marshal;
mod runtime;

use std::convert::TryFrom;
use std::error::Error;
use std::ffi::{CStr, CString};
use std::path::PathBuf;
use std::sync::{Arc, Mutex, RwLock};

use cursed::{FromForeign, InputType, ReturnType, ToForeign};
use once_cell::sync::Lazy;
use pahkat_types::payload::{
    macos::InstallTarget as MacOSInstallTarget, windows::InstallTarget as WindowsInstallTarget,
};
use serde::de::DeserializeOwned;
use serde::Serialize;
use url::Url;

use crate::config::ConfigPath;
use crate::repo::PayloadError;
use crate::transaction::{PackageStatus, PackageStatusError};
use crate::{Config, PackageKey};

use self::log::ExternalLogger;
use marshal::{JsonMarshaler, JsonRefMarshaler, PackageKeyMarshaler, TargetMarshaler};
use runtime::block_on;

#[cthulhu::invoke(return_marshaler = "cursed::UnitMarshaler")]
pub extern "C" fn pahkat_set_logging_callback(
    callback: extern "C" fn(u8, *const libc::c_char, *const libc::c_char, *const libc::c_char),
) -> Result<(), Box<dyn Error>> {
    ::log::set_boxed_logger(Box::new(ExternalLogger { callback }))
        .map(|_| ::log::set_max_level(::log::LevelFilter::Trace))
        .box_err()
}

#[cthulhu::invoke(return_marshaler = "JsonMarshaler")]
pub extern "C" fn pahkat_config_repos_get(
    #[marshal(cursed::ArcRefMarshaler::<RwLock<Config>>)] handle: Arc<RwLock<Config>>,
) -> crate::config::ReposData {
    let config = handle.read().unwrap();
    config.repos().data().clone()
}

#[cthulhu::invoke(return_marshaler = "cursed::UnitMarshaler")]
pub extern "C" fn pahkat_config_repos_set(
    #[marshal(cursed::ArcRefMarshaler::<RwLock<Config>>)] handle: Arc<RwLock<Config>>,
    #[marshal(JsonRefMarshaler::<'_>)] repos: crate::config::ReposData,
) -> Result<(), Box<dyn Error>> {
    handle.write().unwrap().repos_mut().set(repos).box_err()
}

#[cthulhu::invoke(return_marshaler = "cursed::PathBufMarshaler")]
pub extern "C" fn pahkat_config_settings_config_dir(
    #[marshal(cursed::ArcRefMarshaler::<RwLock<Config>>)] handle: Arc<RwLock<Config>>,
) -> std::path::PathBuf {
    handle.read().unwrap().settings().config_dir().to_path_buf()
}

// #[cthulhu::invoke(return_marshaler = "cursed::StringMarshaler")]
// pub extern "C" fn pahkat_config_settings_cache_url(
//     #[marshal(cursed::ArcRefMarshaler::<RwLock<Config>>)] handle: Arc<RwLock<Config>>,
// ) -> String {
//     handle
//         .read()
//         .unwrap()
//         .settings()
//         .cache_base_dir()
//         .as_url()
//         .to_owned()
// }

// #[cthulhu::invoke(return_marshaler = "cursed::UnitMarshaler")]
// pub extern "C" fn pahkat_config_set_cache_url(
//     #[marshal(cursed::ArcRefMarshaler::<RwLock<Config>>)] handle: Arc<RwLock<Config>>,
//     #[marshal(cursed::UrlMarshaler)] url: Url,
// ) -> Result<(), Box<dyn Error>> {
//     let path = ConfigPath::from_url(url)?;
//     handle.write().unwrap().settings().set_cache_base_dir(path)
// }

#[cfg(target_os = "android")]
#[cthulhu::invoke]
pub extern "C" fn pahkat_android_init(
    #[marshal(cursed::PathBufMarshaler)] container_path: PathBuf,
) {
    pathos::android::user::set_container_path(container_path);

    std::panic::set_hook(Box::new(|info| {
        if let Some(s) = info.payload().downcast_ref::<&str>() {
            ::log::error!("{}", s);
        } else if let Some(s) = info.payload().downcast_ref::<String>() {
            ::log::error!("{}", s);
        }

        format!("{:?}", backtrace::Backtrace::new())
            .split("\n")
            .for_each(|x| ::log::error!("{}", x));
    }));
}

#[no_mangle]
pub extern "C" fn pahkat_str_free(ptr: *const libc::c_char) {
    if !ptr.is_null() {
        unsafe { CString::from_raw(ptr as *mut _) };
    }
}

trait BoxError {
    type Item;

    fn box_err(self) -> Result<Self::Item, Box<dyn Error>>;
}

impl<T, E: std::error::Error + 'static> BoxError for Result<T, E> {
    type Item = T;

    #[inline(always)]
    fn box_err(self) -> Result<Self::Item, Box<dyn Error>> {
        self.map_err(|e| Box::new(e) as _)
    }
}
