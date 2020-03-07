use crate::config::ConfigPath;
use directories::BaseDirs;
use once_cell::sync::OnceCell;
use std::path::PathBuf;
use url::Url;

#[cfg(not(target_os = "android"))]
pub fn config_path() -> Option<PathBuf> {
    BaseDirs::new().map(|x| x.config_dir().join("Pahkat"))
}

#[inline(always)]
#[cfg(not(target_os = "android"))]
fn raw_cache_dir() -> Option<PathBuf> {
    BaseDirs::new().map(|x| x.cache_dir().join("Pahkat"))
}

#[cfg(not(any(target_os = "ios", target_os = "android")))]
pub fn cache_dir() -> ConfigPath {
    ConfigPath::File(Url::from_directory_path(&raw_cache_dir().unwrap()).unwrap())
}

#[cfg(not(any(target_os = "ios", target_os = "android")))]
pub fn tmp_dir() -> ConfigPath {
    let url = Url::from_file_path(&raw_cache_dir().unwrap().join("tmp")).unwrap();
    ConfigPath::File(url)
}

#[cfg(target_os = "ios")]
pub fn cache_dir() -> ConfigPath {
    let url = Url::parse("container:/Library/Caches/Pahkat").unwrap();
    ConfigPath::Container(url)
}

#[cfg(target_os = "ios")]
pub fn tmp_dir() -> ConfigPath {
    let url = Url::parse("container:/Library/Caches/Pahkat/tmp").unwrap();
    ConfigPath::Container(url)
}

#[cfg(target_os = "android")]
pub fn cache_dir() -> ConfigPath {
    let url = Url::parse("container:/cache/Pahkat").unwrap();
    ConfigPath::Container(url)
}

#[cfg(target_os = "android")]
pub fn tmp_dir() -> ConfigPath {
    let url = Url::from_directory_path(std::env::temp_dir()).unwrap();
    ConfigPath::File(url)
}

#[cfg(target_os = "macos")]
pub fn uninstall_path() -> PathBuf {
    BaseDirs::new()
        .expect("base directories must be known")
        .data_dir()
        .join("Pahkat")
        .join("uninstall")
}

#[inline(always)]
pub(crate) const fn platform() -> &'static str {
    #[cfg(windows)]
    {
        "windows"
    }
    #[cfg(target_os = "macos")]
    {
        "macos"
    }
}

#[inline(always)]
pub(crate) const fn arch() -> Option<&'static str> {
    #[cfg(target_arch = "x86_64")]
    {
        Some("x86_64")
    }
    #[cfg(target_arch = "x86")]
    {
        Some("x86")
    }
}

#[inline(always)]
pub(crate) fn payloads() -> &'static [&'static str] {
    #[cfg(windows)]
    {
        &["WindowsExecutable"]
    }
    #[cfg(target_os = "macos")]
    {
        &["MacOSPackage"]
    }
}
