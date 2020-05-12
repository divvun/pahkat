pub mod macos;
pub mod tarball;
pub mod windows;

use std::collections::BTreeSet;
use std::convert::TryFrom;
use std::str::FromStr;

use crate::DependencyMap;
use serde::{Deserialize, Serialize};
use typed_builder::TypedBuilder;

pub(crate) fn parse_set<T: FromStr + Ord>(s: &str) -> Result<BTreeSet<T>, T::Err> {
    if s == "" {
        return Ok(BTreeSet::new());
    }
    s.split(",").map(|x| T::from_str(x.trim())).collect::<Result<BTreeSet<T>, _>>()
} 

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[serde(untagged)] // #[serde(tag = "_type")]
#[non_exhaustive]
#[cfg_attr(feature = "structopt", derive(structopt::StructOpt))]
pub enum Payload {
    WindowsExecutable(windows::Executable),
    #[cfg_attr(feature = "structopt", structopt(name = "macos-package"))]
    MacOSPackage(macos::Package),
    TarballPackage(tarball::Package),
}

impl Payload {
    pub fn size(&self) -> u64 {
        match self {
            Payload::WindowsExecutable(x) => x.size,
            Payload::MacOSPackage(x) => x.size,
            Payload::TarballPackage(x) => x.size,
        }
    }

    pub fn installed_size(&self) -> u64 {
        match self {
            Payload::WindowsExecutable(x) => x.installed_size,
            Payload::MacOSPackage(x) => x.installed_size,
            Payload::TarballPackage(x) => x.installed_size,
        }
    }
}

#[derive(
    Debug, Serialize, Deserialize, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, TypedBuilder,
)]
pub struct Target {
    pub platform: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[builder(default)]
    pub arch: Option<String>,
    #[serde(default)]
    #[builder(default)]
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
