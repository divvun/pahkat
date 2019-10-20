use crate::store_config::ConfigPath;
use directories::BaseDirs;
use std::path::PathBuf;
use url::Url;

pub fn config_path() -> PathBuf {
    BaseDirs::new()
        .expect("base directories must be known")
        .config_dir()
        .join("Pahkat")
}

fn raw_cache_path() -> PathBuf {
    BaseDirs::new()
        .expect("base directories must be known")
        .cache_dir()
        .join("Pahkat")
}

#[cfg(not(target_os = "ios"))]
pub fn cache_path() -> ConfigPath {
    ConfigPath::File(Url::from_directory_path(&raw_cache_path()).unwrap())
}

#[cfg(not(target_os = "ios"))]
pub fn tmp_path() -> ConfigPath {
    let url = Url::from_file_path(&raw_cache_path().join("tmp")).unwrap();
    ConfigPath::File(url)
}

#[cfg(target_os = "ios")]
pub fn cache_path() -> ConfigPath {
    let url = Url::parse("container:Caches/Pahkat").unwrap();
    ConfigPath::Container(url);
}

#[cfg(target_os = "ios")]
pub fn tmp_path() -> ConfigPath {
    let url = Url::parse("container:Caches/Pahkat/tmp").unwrap();
    ConfigPath::Container(url);
}

#[cfg(target_os = "macos")]
pub fn uninstall_path() -> PathBuf {
    BaseDirs::new()
        .expect("base directories must be known")
        .data_dir()
        .join("Pahkat")
        .join("uninstall")
}
