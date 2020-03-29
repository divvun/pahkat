#[cfg(all(target_os = "macos", feature = "macos"))]
pub mod macos;

#[cfg(all(windows, feature = "windows"))]
pub mod windows;

#[cfg(feature = "prefix")]
pub mod prefix;

use std::convert::TryFrom;
use std::error::Error;
use std::ffi::{CStr, CString};
use std::path::PathBuf;
use std::sync::{Arc, RwLock};

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

pub struct TargetMarshaler;

impl InputType for TargetMarshaler {
    type Foreign = <cursed::StringMarshaler as InputType>::Foreign;
}

impl ReturnType for TargetMarshaler {
    type Foreign = cursed::Slice<u8>;

    fn foreign_default() -> Self::Foreign {
        cursed::Slice::default()
    }
}

impl ToForeign<MacOSInstallTarget, cursed::Slice<u8>> for TargetMarshaler {
    type Error = std::convert::Infallible;

    fn to_foreign(input: MacOSInstallTarget) -> Result<cursed::Slice<u8>, Self::Error> {
        let str_target = match input {
            MacOSInstallTarget::System => "system",
            MacOSInstallTarget::User => "user",
        };

        cursed::StringMarshaler::to_foreign(str_target.to_string())
    }
}

impl FromForeign<cursed::Slice<u8>, MacOSInstallTarget> for TargetMarshaler {
    type Error = Box<dyn Error>;

    unsafe fn from_foreign(ptr: cursed::Slice<u8>) -> Result<MacOSInstallTarget, Self::Error> {
        let str_target = cursed::StringMarshaler::from_foreign(ptr)?;

        Ok(match &*str_target {
            "user" => MacOSInstallTarget::User,
            _ => MacOSInstallTarget::System,
        })
    }
}

impl ToForeign<WindowsInstallTarget, cursed::Slice<u8>> for TargetMarshaler {
    type Error = std::convert::Infallible;

    fn to_foreign(input: WindowsInstallTarget) -> Result<cursed::Slice<u8>, Self::Error> {
        let str_target = match input {
            WindowsInstallTarget::System => "system",
            WindowsInstallTarget::User => "user",
        };

        cursed::StringMarshaler::to_foreign(str_target.to_string())
    }
}

impl FromForeign<cursed::Slice<u8>, WindowsInstallTarget> for TargetMarshaler {
    type Error = Box<dyn Error>;

    unsafe fn from_foreign(ptr: cursed::Slice<u8>) -> Result<WindowsInstallTarget, Self::Error> {
        let str_target = cursed::StringMarshaler::from_foreign(ptr)?;

        Ok(match &*str_target {
            "user" => WindowsInstallTarget::User,
            _ => WindowsInstallTarget::System,
        })
    }
}

#[inline(always)]
fn level_u8_to_str(level: u8) -> Option<&'static str> {
    Some(match level {
        0 => return None,
        1 => "error",
        2 => "warn",
        3 => "info",
        4 => "debug",
        _ => "trace",
    })
}

#[cfg(not(target_os = "android"))]
#[no_mangle]
pub extern "C" fn pahkat_enable_logging(level: u8) {
    use std::io::Write;

    if let Some(level) = level_u8_to_str(level) {
        std::env::set_var("RUST_LOG", format!("pahkat_client={}", level));
    }

    env_logger::builder()
        .format(|buf, record| {
            writeln!(
                buf,
                "{} {} {}:{} > {}",
                record.level(),
                record.target(),
                record.file().unwrap_or("<unknown>"),
                record.line().unwrap_or(0),
                record.args()
            )
        })
        .init();
}

#[cfg(target_os = "android")]
#[no_mangle]
pub extern "C" fn pahkat_enable_logging(level: u8) {
    use std::io::Write;

    if let Some(level) = level_u8_to_str(level) {
        std::env::set_var("RUST_LOG", format!("pahkat_client={}", level));
    }

    let mut derp = android_log::LogBuilder::new("PahkatClient");
    derp.format(|record| {
        format!(
            "{} {} {}:{} > {}",
            record.level(),
            record.target(),
            record.file().unwrap_or("<unknown>"),
            record.line().unwrap_or(0),
            record.args()
        )
    });
    derp.init().unwrap();
}

pub struct JsonMarshaler;

impl InputType for JsonMarshaler {
    type Foreign = <cursed::StringMarshaler as InputType>::Foreign;
}

impl ReturnType for JsonMarshaler {
    type Foreign = cursed::Slice<u8>;

    fn foreign_default() -> Self::Foreign {
        cursed::Slice::default()
    }
}

impl<T> ToForeign<T, cursed::Slice<u8>> for JsonMarshaler
where
    T: Serialize,
{
    type Error = Box<dyn Error>;

    fn to_foreign(input: T) -> Result<cursed::Slice<u8>, Self::Error> {
        let json_str = serde_json::to_string(&input)?;
        Ok(cursed::StringMarshaler::to_foreign(json_str).unwrap())
    }
}

impl<T> FromForeign<cursed::Slice<u8>, T> for JsonMarshaler
where
    T: DeserializeOwned,
{
    type Error = Box<dyn Error>;

    unsafe fn from_foreign(ptr: cursed::Slice<u8>) -> Result<T, Self::Error> {
        let json_str = cursed::StringMarshaler::from_foreign(ptr)?;
        log::debug!("JSON: {}, type: {}", &json_str, std::any::type_name::<T>());

        let v: Result<T, _> = serde_json::from_str(&json_str);
        v.map_err(|e| {
            log::error!("Json error: {}", &e);
            log::debug!("{:?}", &e);
            Box::new(e) as _
        })
    }
}

pub struct PackageKeyMarshaler;

impl InputType for PackageKeyMarshaler {
    type Foreign = <cursed::StringMarshaler as InputType>::Foreign;
}

impl ReturnType for PackageKeyMarshaler {
    type Foreign = <cursed::StringMarshaler as ReturnType>::Foreign;

    fn foreign_default() -> Self::Foreign {
        Default::default()
    }
}

impl<'a> ToForeign<&'a PackageKey, cursed::Slice<u8>> for PackageKeyMarshaler {
    type Error = std::convert::Infallible;

    fn to_foreign(key: &'a PackageKey) -> Result<cursed::Slice<u8>, Self::Error> {
        cursed::StringMarshaler::to_foreign(key.to_string())
    }
}

impl FromForeign<cursed::Slice<u8>, PackageKey> for PackageKeyMarshaler {
    type Error = Box<dyn Error>;

    unsafe fn from_foreign(string: cursed::Slice<u8>) -> Result<PackageKey, Self::Error> {
        let s = cursed::StringMarshaler::from_foreign(string)?;
        PackageKey::try_from(&*s).box_err()
    }
}

struct ExternalLogger {
    callback: LoggingCallback,
}

fn make_unknown_cstr() -> CString {
    CString::new("<unknown>").unwrap()
}

impl log::Log for ExternalLogger {
    fn enabled(&self, metadata: &log::Metadata) -> bool {
        true
    }

    fn log(&self, record: &log::Record) {
        use log::Level::*;

        let level = match record.level() {
            Error => 1,
            Warn => 2,
            Info => 3,
            Debug => 4,
            Trace => 5,
        };

        let msg =
            CString::new(format!("{}", record.args())).unwrap_or_else(|_| make_unknown_cstr());
        let module = CString::new(record.module_path().unwrap_or("<unknown>"))
            .unwrap_or_else(|_| make_unknown_cstr());
        let file_path = format!(
            "{}:{}",
            record.file().unwrap_or("<unknown>"),
            record.line().unwrap_or(0)
        );
        let file_path = CString::new(file_path).unwrap_or_else(|_| make_unknown_cstr());

        (self.callback)(level, msg.as_ptr(), module.as_ptr(), file_path.as_ptr());
    }

    fn flush(&self) {}
}

type LoggingCallback =
    extern "C" fn(u8, *const libc::c_char, *const libc::c_char, *const libc::c_char);
// static LOGGING_CALLBACK: Lazy<RwLock<Option<Box<ExternalLogger>>> = Lazy::new(|| RwLock::new(None));

#[cthulhu::invoke(return_marshaler = "cursed::UnitMarshaler")]
pub extern "C" fn pahkat_set_logging_callback(
    callback: extern "C" fn(u8, *const libc::c_char, *const libc::c_char, *const libc::c_char),
) -> Result<(), Box<dyn Error>> {
    log::set_boxed_logger(Box::new(ExternalLogger { callback }))
        .map(|_| log::set_max_level(log::LevelFilter::Trace))
        .box_err()
}

#[cthulhu::invoke(return_marshaler = "JsonMarshaler")]
pub extern "C" fn pahkat_config_repos_get(
    #[marshal(cursed::ArcRefMarshaler::<RwLock<Config>>)] handle: Arc<RwLock<Config>>,
) -> crate::config::ReposData {
    let config = handle.read().unwrap();
    config.repos().get().clone()
}

#[cthulhu::invoke(return_marshaler = "cursed::UnitMarshaler")]
pub extern "C" fn pahkat_config_repos_set(
    #[marshal(cursed::ArcRefMarshaler::<RwLock<Config>>)] handle: Arc<RwLock<Config>>,
    #[marshal(JsonMarshaler)] repos: crate::config::ReposData,
) -> Result<(), Box<dyn Error>> {
    handle.write().unwrap().repos_mut().set(repos).box_err()
}

#[cthulhu::invoke(return_marshaler = "cursed::PathBufMarshaler")]
pub extern "C" fn pahkat_config_settings_config_dir(
    #[marshal(cursed::ArcRefMarshaler::<RwLock<Config>>)] handle: Arc<RwLock<Config>>,
) -> std::path::PathBuf {
    handle.read().unwrap().settings().config_dir().to_path_buf()
}

#[cthulhu::invoke(return_marshaler = "cursed::UrlMarshaler")]
pub extern "C" fn pahkat_config_settings_cache_url(
    #[marshal(cursed::ArcRefMarshaler::<RwLock<Config>>)] handle: Arc<RwLock<Config>>,
) -> Url {
    handle
        .read()
        .unwrap()
        .settings()
        .cache_base_dir()
        .as_url()
        .to_owned()
}

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
    let _ = crate::config::CONTAINER_PATH.set(container_path).ok();

    std::panic::set_hook(Box::new(|info| {
        if let Some(s) = info.payload().downcast_ref::<&str>() {
            log::error!("{}", s);
        } else if let Some(s) = info.payload().downcast_ref::<String>() {
            log::error!("{}", s);
        }

        format!("{:?}", backtrace::Backtrace::new())
            .split("\n")
            .for_each(|x| log::error!("{}", x));
    }));
}

#[no_mangle]
pub extern "C" fn pahkat_str_free(ptr: *const libc::c_char) {
    if !ptr.is_null() {
        unsafe { CString::from_raw(ptr as *mut _) };
    }
}

#[inline(always)]
pub(crate) fn status_to_i8(result: Result<PackageStatus, PackageStatusError>) -> i8 {
    match result {
        Ok(status) => match status {
            PackageStatus::NotInstalled => 0,
            PackageStatus::UpToDate => 1,
            PackageStatus::RequiresUpdate => 2,
        },
        Err(error) => match error {
            PackageStatusError::Payload(e) => match e {
                PayloadError::NoPackage | PayloadError::NoConcretePackage => -1,
                PayloadError::NoPayloadFound => -2,
                PayloadError::CriteriaUnmet(_) => -5,
            },
            PackageStatusError::WrongPayloadType => -3,
            PackageStatusError::ParsingVersion => -4,
        },
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
