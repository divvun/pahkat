use crate::config::ConfigPath;
use std::path::{Path, PathBuf};
use url::Url;

const APP_PATH: &str = "Pahkat";

#[cfg(not(target_os = "android"))]
pub fn config_path() -> PathBuf {
    if cfg!(windows) && whoami::username() == "SYSTEM" {
        return pathos::system::app_config_dir(APP_PATH)
    }
    
    if cfg!(target_os = "macos") && whoami::username() == "root" {
        return pathos::system::app_config_dir(APP_PATH)
    }
    
    pathos::user::app_config_dir(APP_PATH)
}

#[inline(always)]
#[cfg(not(target_os = "android"))]
fn raw_cache_dir() -> PathBuf {
    if cfg!(windows) && whoami::username() == "SYSTEM" {
        return pathos::system::app_cache_dir(APP_PATH)
    }
    
    if cfg!(target_os = "macos") && whoami::username() == "root" {
        return pathos::system::app_cache_dir(APP_PATH)
    }

    pathos::user::app_cache_dir(APP_PATH)
}

#[inline(always)]
pub fn log_path() -> PathBuf {
    if cfg!(windows) && whoami::username() == "SYSTEM" {
        return pathos::system::app_log_dir(APP_PATH)
    }
    
    if cfg!(target_os = "macos") && whoami::username() == "root" {
        return pathos::system::app_log_dir(APP_PATH)
    }

    return pathos::user::app_log_dir(APP_PATH)
}

pub fn cache_dir() -> ConfigPath {
    ConfigPath(pathos::user::iri::app_cache_dir(APP_PATH))
}

pub fn tmp_dir() -> ConfigPath {
    ConfigPath(pathos::user::iri::app_temporary_dir(APP_PATH))
}

#[cfg(target_os = "macos")]
pub fn uninstall_path() -> PathBuf {
    pathos::user::app_data_dir(APP_PATH).join("uninstall")
}

macro_rules! platform {
    ($name:expr) => {{
        #[cfg(target_os = $name)]
        {
            return $name;
        }
    }};
}

#[inline(always)]
#[allow(unreachable_code)]
pub(crate) const fn platform() -> &'static str {
    platform!("windows");
    platform!("macos");
    platform!("ios");
    platform!("android");
    platform!("linux");
}

macro_rules! arch {
    ($name:expr) => {{
        #[cfg(target_arch = $name)]
        {
            return Some($name);
        }
    }};
}

#[inline(always)]
#[allow(unreachable_code)]
pub(crate) const fn arch() -> Option<&'static str> {
    arch!("x86_64");
    arch!("x86");
    arch!("arm");
    arch!("aarch64");
    arch!("mips");
    arch!("mips64");
    arch!("powerpc");
    arch!("powerpc64");
}

#[inline(always)]
pub(crate) fn payloads() -> &'static [&'static str] {
    #[cfg(all(feature = "windows", not(feature = "macos"), not(feature = "prefix")))]
    {
        &["WindowsExecutable"]
    }
    #[cfg(all(not(feature = "windows"), feature = "macos", not(feature = "prefix")))]
    {
        &["MacOSPackage"]
    }
    #[cfg(all(not(feature = "windows"), not(feature = "macos"), feature = "prefix"))]
    {
        &["TarballPackage"]
    }

    #[cfg(all(
        not(feature = "windows"),
        not(feature = "macos"),
        not(feature = "prefix")
    ))]
    compile_error!("One of the above features must be enabled");
}
