use crate::config::ConfigPath;
use std::path::{Path, PathBuf};
use url::Url;
use once_cell::sync::Lazy;
// use

const APP_PATH: &str = "Pahkat";


use pathos::AppDirs as _;

static SYS_DIRS: Lazy<Result<pathos::system::AppDirs, pathos::Error>> = Lazy::new(|| pathos::system::AppDirs::new(APP_PATH));
static USER_DIRS: Lazy<Result<pathos::user::AppDirs, pathos::Error>> = Lazy::new(|| pathos::user::AppDirs::new(APP_PATH));

macro_rules! sys_dir {
    (| $x:ident | $($input:tt)*) => {
        SYS_DIRS.as_ref()
            .map(|$x| $($input)*)
            .map_err(|e| e.clone())
    }
}

macro_rules! user_dir {
    (| $x:ident | $($input:tt)*) => {
        USER_DIRS.as_ref()
            .map(|$x| $($input)*)
            .map_err(|e| e.clone())
    }
}

#[cfg(not(target_os = "android"))]
pub fn config_path() -> Result<&'static Path, pathos::Error> {
    #[cfg(windows)]
    {
        if cfg!(windows) && whoami::username() == "SYSTEM" {
            return sys_dir!(|x| x.config_dir());
        }
    }

    #[cfg(target_os = "macos")]
    {
        if cfg!(target_os = "macos") && whoami::username() == "root" {
            return sys_dir!(|x| x.config_dir());
        }
    }

    return user_dir!(|x| x.config_dir());
}

#[inline(always)]
#[cfg(not(target_os = "android"))]
fn raw_cache_dir() -> Result<&'static Path, pathos::Error> {
    #[cfg(windows)]
    {
        if whoami::username() == "SYSTEM" {
            return sys_dir!(|x| x.cache_dir());
        }
    }

    #[cfg(target_os = "macos")]
    {
        if whoami::username() == "root" {
            return sys_dir!(|x| x.cache_dir());
        }
    }

    return user_dir!(|x| x.cache_dir());
}

#[inline(always)]
#[cfg(not(target_os = "android"))]
pub fn log_path() -> Result<&'static Path, pathos::Error> {
    #[cfg(windows)]
    {
        if whoami::username() == "SYSTEM" {
            return sys_dir!(|x| x.log_dir());
        }
    }

    #[cfg(target_os = "macos")]
    {
        if whoami::username() == "root" {
            return sys_dir!(|x| x.log_dir());
        }
    }

    return user_dir!(|x| x.log_dir());
}

pub fn cache_dir() -> Result<ConfigPath, pathos::Error> {
    #[cfg(windows)]
    {
        if whoami::username() == "SYSTEM" {
            return Ok(ConfigPath(pathos::system::iri::app_cache_dir(APP_PATH)?));
        }
    }

    #[cfg(target_os = "macos")]
    {
        if cfg!(target_os = "macos") && whoami::username() == "root" {
            return Ok(ConfigPath(pathos::system::iri::app_cache_dir(APP_PATH)?));
        }
    }

    Ok(ConfigPath(pathos::user::iri::app_cache_dir(APP_PATH)?))
}

pub fn tmp_dir() -> Result<ConfigPath, pathos::Error> {
    #[cfg(windows)]
    {
        if whoami::username() == "SYSTEM" {
            return Ok(ConfigPath(pathos::system::iri::app_temporary_dir(APP_PATH)?));
        }
    }

    #[cfg(target_os = "macos")]
    {
        if whoami::username() == "root" {
            return Ok(ConfigPath(pathos::system::iri::app_temporary_dir(APP_PATH)?));
        }
    }

    Ok(ConfigPath(pathos::user::iri::app_temporary_dir(APP_PATH)?))
}

#[cfg(all(target_os = "macos", feature = "launchd"))]
pub fn uninstall_path() -> Result<PathBuf, pathos::Error> {
    if whoami::username() == "root" {
        return Ok(pathos::system::app_data_dir(APP_PATH)?.join("uninstall"));
    }
    Ok(pathos::user::app_data_dir(APP_PATH)?.join("uninstall"))
}

#[cfg(all(target_os = "macos", not(feature = "launchd")))]
pub fn uninstall_path() -> Result<PathBuf, pathos::Error> {
    Ok(pathos::user::app_data_dir(APP_PATH)?.join("uninstall"))
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
