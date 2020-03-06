pub mod macos;
pub mod tarball;
pub mod windows;

use crate::DependencyMap;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(untagged)]
pub enum Payload {
    WindowsExecutable(windows::Executable),
    MacOSPackage(macos::Package),
    TarballPackage(tarball::Package),
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Target {
    pub platform: String,
    pub arch: Option<String>,
    #[serde(default = "DependencyMap::new")]
    pub dependencies: DependencyMap,
    pub payload: Payload,
}

pub trait AsDownloadUrl {
    fn as_download_url(&self) -> &url::Url;
}
