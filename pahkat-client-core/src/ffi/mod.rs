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
use pahkat_types::InstallTarget;
use serde::de::DeserializeOwned;
use serde::Serialize;
use url::Url;

use crate::repo::RepoRecord;
use crate::transaction::{PackageStatus, PackageStatusError};
use crate::{PackageKey, StoreConfig};

pub struct TargetMarshaler;

impl InputType for TargetMarshaler {
    type Foreign = <cursed::StringMarshaler as InputType>::Foreign;
}

impl ReturnType for TargetMarshaler {
    type Foreign = *const libc::c_char;

    fn foreign_default() -> Self::Foreign {
        std::ptr::null()
    }
}

impl ToForeign<InstallTarget, *const libc::c_char> for TargetMarshaler {
    type Error = Box<dyn Error>;

    fn to_foreign(input: InstallTarget) -> Result<*const libc::c_char, Self::Error> {
        let str_target = match input {
            InstallTarget::System => "system",
            InstallTarget::User => "user",
        };

        let c_str = CString::new(str_target)?;
        Ok(c_str.into_raw())
    }
}

impl FromForeign<*const libc::c_char, InstallTarget> for TargetMarshaler {
    type Error = Box<dyn Error>;

    fn from_foreign(ptr: *const libc::c_char) -> Result<InstallTarget, Self::Error> {
        if ptr.is_null() {
            return Err(cursed::null_ptr_error());
        }

        let s = unsafe { CStr::from_ptr(ptr) }.to_str()?;
        Ok(match s {
            "user" => InstallTarget::User,
            _ => InstallTarget::System,
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
    type Foreign = *const libc::c_char;

    fn foreign_default() -> Self::Foreign {
        std::ptr::null()
    }
}

impl<T> ToForeign<T, *const libc::c_char> for JsonMarshaler
where
    T: Serialize,
{
    type Error = Box<dyn Error>;

    fn to_foreign(input: T) -> Result<*const libc::c_char, Self::Error> {
        let vec = serde_json::to_vec(&input)?;
        let c_str = CString::new(vec)?;
        log::debug!("JSON MARSHAL: {:?}", &c_str);
        Ok(c_str.into_raw())
    }

    // fn drop_foreign(ptr: *const c_char) {
    //     unsafe { CString::from_raw(ptr as *mut _) };
    // }
}

impl<T> FromForeign<*const libc::c_char, T> for JsonMarshaler
where
    T: DeserializeOwned,
{
    type Error = Box<dyn Error>;

    fn from_foreign(ptr: *const libc::c_char) -> Result<T, Self::Error> {
        if ptr.is_null() {
            return Err(cursed::null_ptr_error());
        }

        let s = unsafe { CStr::from_ptr(ptr) }.to_str()?;
        log::debug!("JSON: {}, type: {}", s, std::any::type_name::<T>());
        let v: Result<T, _> = serde_json::from_str(&s);
        v.map_err(|e| {
            log::error!("Json error: {}", &e);
            log::debug!("{:?}", &e);
            Box::new(e) as _
        })
    }

    // fn drop_local(ptr: *const c_char) {
    //     unsafe { CString::from_raw(ptr as *mut _) };
    // }
}

pub struct PackageKeyMarshaler;

impl InputType for PackageKeyMarshaler {
    type Foreign = <cursed::StringMarshaler as InputType>::Foreign;
}

impl ReturnType for PackageKeyMarshaler {
    type Foreign = <cursed::StringMarshaler as ReturnType>::Foreign;

    fn foreign_default() -> Self::Foreign {
        std::ptr::null()
    }
}

impl<'a> ToForeign<&'a PackageKey, *const libc::c_char> for PackageKeyMarshaler {
    type Error = Box<dyn Error>;

    fn to_foreign(key: &'a PackageKey) -> Result<*const libc::c_char, Self::Error> {
        cursed::StringMarshaler::to_foreign(key.to_string())
    }
}

impl FromForeign<*const libc::c_char, PackageKey> for PackageKeyMarshaler {
    type Error = Box<dyn Error>;

    fn from_foreign(string: *const libc::c_char) -> Result<PackageKey, Self::Error> {
        if string.is_null() {
            return Err(cursed::null_ptr_error());
        }

        let s: &str = cursed::StrMarshaler::from_foreign(string)?;
        PackageKey::try_from(s).map_err(|e| Box::new(e) as _)
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
        .map_err(|err| Box::new(err) as _)
}

#[cthulhu::invoke(return_marshaler = "cursed::UnitMarshaler")]
pub extern "C" fn pahkat_config_set_ui_value(
    #[marshal(cursed::ArcRefMarshaler::<RwLock<StoreConfig>>)] handle: Arc<RwLock<StoreConfig>>,
    #[marshal(cursed::StrMarshaler)] key: &str,
    #[marshal(cursed::StrMarshaler)] value: Option<&str>,
) -> Result<(), Box<dyn Error>> {
    let config = handle.write().unwrap();
    config.set_ui_value(key, value.map(|x| x.to_string()))
}

#[cthulhu::invoke(return_marshaler = "cursed::StringMarshaler")]
pub extern "C" fn pahkat_config_ui_value(
    #[marshal(cursed::ArcRefMarshaler::<RwLock<StoreConfig>>)] handle: Arc<RwLock<StoreConfig>>,
    #[marshal(cursed::StrMarshaler)] key: &str,
) -> Option<String> {
    let config = handle.read().unwrap();
    config.ui_value(key)
}

#[cthulhu::invoke(return_marshaler = "cursed::StringMarshaler")]
pub extern "C" fn pahkat_config_skipped_package(
    #[marshal(cursed::ArcRefMarshaler::<RwLock<StoreConfig>>)] handle: Arc<RwLock<StoreConfig>>,
    #[marshal(PackageKeyMarshaler)] key: PackageKey,
) -> Option<String> {
    let config = handle.read().unwrap();
    config.skipped_package(&key)
}

#[cthulhu::invoke(return_marshaler = "cursed::UnitMarshaler")]
pub extern "C" fn pahkat_config_add_skipped_package(
    #[marshal(cursed::ArcRefMarshaler::<RwLock<StoreConfig>>)] handle: Arc<RwLock<StoreConfig>>,
    #[marshal(PackageKeyMarshaler)] key: PackageKey,
    #[marshal(cursed::StrMarshaler)] version: &str,
) -> Result<(), Box<dyn Error>> {
    let config = handle.read().unwrap();
    config.add_skipped_package(key, version.into())
}

#[cthulhu::invoke(return_marshaler = "JsonMarshaler")]
pub extern "C" fn pahkat_config_repos(
    #[marshal(cursed::ArcRefMarshaler::<RwLock<StoreConfig>>)] handle: Arc<RwLock<StoreConfig>>,
) -> Vec<RepoRecord> {
    let config = handle.read().unwrap();
    config.repos()
}

#[cthulhu::invoke(return_marshaler = "cursed::UnitMarshaler")]
pub extern "C" fn pahkat_config_set_repos(
    #[marshal(cursed::ArcRefMarshaler::<RwLock<StoreConfig>>)] handle: Arc<RwLock<StoreConfig>>,
    #[marshal(JsonMarshaler)] repos: Vec<RepoRecord>,
) -> Result<(), Box<dyn Error>> {
    handle.write().unwrap().set_repos(repos)
}

#[cthulhu::invoke(return_marshaler = "cursed::PathMarshaler")]
pub extern "C" fn pahkat_config_config_path(
    #[marshal(cursed::ArcRefMarshaler::<RwLock<StoreConfig>>)] handle: Arc<RwLock<StoreConfig>>,
) -> std::path::PathBuf {
    handle.read().unwrap().config_path().to_path_buf()
}

#[cthulhu::invoke(return_marshaler = "cursed::UrlMarshaler")]
pub extern "C" fn pahkat_config_cache_base_url(
    #[marshal(cursed::ArcRefMarshaler::<RwLock<StoreConfig>>)] handle: Arc<RwLock<StoreConfig>>,
) -> Url {
    handle.read().unwrap().cache_base_dir().as_url().to_owned()
}

use crate::config::ConfigPath;

#[cthulhu::invoke(return_marshaler = "cursed::UnitMarshaler")]
pub extern "C" fn pahkat_config_set_cache_base_url(
    #[marshal(cursed::ArcRefMarshaler::<RwLock<StoreConfig>>)] handle: Arc<RwLock<StoreConfig>>,
    #[marshal(cursed::UrlMarshaler)] url: Url,
) -> Result<(), Box<dyn Error>> {
    let path = ConfigPath::from_url(url)?;
    handle.write().unwrap().set_cache_base_dir(path)
}

#[cfg(target_os = "android")]
#[cthulhu::invoke]
pub extern "C" fn pahkat_android_init(#[marshal(cursed::PathMarshaler)] container_path: PathBuf) {
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
    use PackageStatusError::*;

    match result {
        Ok(status) => match status {
            PackageStatus::NotInstalled => 0,
            PackageStatus::UpToDate => 1,
            PackageStatus::RequiresUpdate => 2,
            PackageStatus::Skipped => 3,
        },
        Err(error) => match error {
            NoPackage => -1,
            NoInstaller => -2,
            WrongInstallerType => -3,
            ParsingVersion => -4,
            InvalidInstallPath => -5,
            InvalidMetadata => -6,
        },
    }
}
