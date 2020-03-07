pub mod macos;
pub mod tarball;
pub mod windows;

use crate::DependencyMap;
use serde::{Deserialize, Serialize};
use std::convert::TryFrom;

#[derive(Debug, Serialize, Deserialize, Clone)]
#[non_exhaustive]
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

impl AsDownloadUrl for Payload {
    fn as_download_url(&self) -> &url::Url {
        use Payload::*;
        match self {
            WindowsExecutable(p) => p.as_download_url(),
            MacOSPackage(p) => p.as_download_url(),
            TarballPackage(p) => p.as_download_url(),
        }
    }
}

impl TryFrom<Payload> for windows::Executable {
    type Error = Payload;

    fn try_from(value: Payload) -> Result<Self, Self::Error> {
        match value {
            Payload::WindowsExecutable(v) => Ok(v),
            x => Err(x),
        }
    }
}

impl<'a> TryFrom<&'a Payload> for &'a windows::Executable {
    type Error = &'a Payload;

    fn try_from(value: &'a Payload) -> Result<Self, Self::Error> {
        match value {
            Payload::WindowsExecutable(v) => Ok(v),
            x => Err(x),
        }
    }
}

impl TryFrom<Payload> for macos::Package {
    type Error = Payload;

    fn try_from(value: Payload) -> Result<Self, Self::Error> {
        match value {
            Payload::MacOSPackage(v) => Ok(v),
            x => Err(x),
        }
    }
}

impl<'a> TryFrom<&'a Payload> for &'a macos::Package {
    type Error = &'a Payload;

    fn try_from(value: &'a Payload) -> Result<Self, Self::Error> {
        match value {
            Payload::MacOSPackage(v) => Ok(v),
            x => Err(x),
        }
    }
}

impl TryFrom<Payload> for tarball::Package {
    type Error = Payload;

    fn try_from(value: Payload) -> Result<Self, Self::Error> {
        match value {
            Payload::TarballPackage(v) => Ok(v),
            x => Err(x),
        }
    }
}

impl<'a> TryFrom<&'a Payload> for &'a tarball::Package {
    type Error = &'a Payload;

    fn try_from(value: &'a Payload) -> Result<Self, Self::Error> {
        match value {
            Payload::TarballPackage(v) => Ok(v),
            x => Err(x),
        }
    }
}
