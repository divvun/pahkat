use crate::store_config::ConfigPath;
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
fn raw_cache_path() -> Option<PathBuf> {
    BaseDirs::new().map(|x| x.cache_dir().join("Pahkat"))
}

#[cfg(not(any(target_os = "ios", target_os = "android")))]
pub fn cache_path() -> ConfigPath {
    ConfigPath::File(Url::from_directory_path(&raw_cache_path().unwrap()).unwrap())
}

#[cfg(not(any(target_os = "ios", target_os = "android")))]
pub fn tmp_path() -> ConfigPath {
    let url = Url::from_file_path(&raw_cache_path().unwrap().join("tmp")).unwrap();
    ConfigPath::File(url)
}

#[cfg(target_os = "ios")]
pub fn cache_path() -> ConfigPath {
    let url = Url::parse("container:/Library/Caches/Pahkat").unwrap();
    ConfigPath::Container(url)
}

#[cfg(target_os = "ios")]
pub fn tmp_path() -> ConfigPath {
    let url = Url::parse("container:/Library/Caches/Pahkat/tmp").unwrap();
    ConfigPath::Container(url)
}

#[cfg(target_os = "android")]
pub fn cache_path() -> ConfigPath {
    let url = Url::parse("container:/cache/Pahkat").unwrap();
    ConfigPath::Container(url)
}

#[cfg(target_os = "android")]
pub fn tmp_path() -> ConfigPath {
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
