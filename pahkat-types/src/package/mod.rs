pub mod version;

use derive_builder::Builder;
use indexmap::IndexMap;
use serde::{Deserialize, Serialize};
use std::convert::TryFrom;
use url::Url;

use crate::LangTagMap;
pub use version::Version;

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(untagged)]
pub enum Package {
    Concrete(Descriptor),
    Synthetic(crate::synth::Descriptor),
    Redirect(Redirect),
}

impl TryFrom<Package> for Descriptor {
    type Error = Package;

    #[inline]
    fn try_from(value: Package) -> Result<Self, Self::Error> {
        match value {
            Package::Concrete(v) => Ok(v),
            x => Err(x),
        }
    }
}

impl<'a> TryFrom<&'a Package> for &'a Descriptor {
    type Error = &'a Package;

    #[inline]
    fn try_from(value: &'a Package) -> Result<Self, Self::Error> {
        match value {
            Package::Concrete(v) => Ok(v),
            x => Err(x),
        }
    }
}

impl TryFrom<Package> for crate::synth::Descriptor {
    type Error = Package;

    #[inline]
    fn try_from(value: Package) -> Result<Self, Self::Error> {
        match value {
            Package::Synthetic(v) => Ok(v),
            x => Err(x),
        }
    }
}

impl<'a> TryFrom<&'a Package> for &'a crate::synth::Descriptor {
    type Error = &'a Package;

    #[inline]
    fn try_from(value: &'a Package) -> Result<Self, Self::Error> {
        match value {
            Package::Synthetic(v) => Ok(v),
            x => Err(x),
        }
    }
}

impl TryFrom<Package> for Redirect {
    type Error = Package;

    #[inline]
    fn try_from(value: Package) -> Result<Self, Self::Error> {
        match value {
            Package::Redirect(v) => Ok(v),
            x => Err(x),
        }
    }
}

impl<'a> TryFrom<&'a Package> for &'a Redirect {
    type Error = &'a Package;

    #[inline]
    fn try_from(value: &'a Package) -> Result<Self, Self::Error> {
        match value {
            Package::Redirect(v) => Ok(v),
            x => Err(x),
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone, Builder)]
#[serde(rename_all = "camelCase")]
pub struct Index {
    #[serde(default = "IndexMap::new")]
    pub packages: IndexMap<String, Package>,
}

#[derive(Debug, Serialize, Deserialize, Clone, Builder)]
#[serde(rename_all = "camelCase")]
pub struct Descriptor {
    #[serde(rename = "_type")]
    _type: String,
    pub id: String,

    #[serde(default = "LangTagMap::new")]
    pub name: LangTagMap<String>,
    #[serde(default = "LangTagMap::new")]
    pub description: LangTagMap<String>,
    #[serde(default = "Vec::new")]
    pub tags: Vec<String>,
    #[serde(default = "Vec::new")]
    pub releases: Vec<Release>,
}

#[derive(Debug, Serialize, Deserialize, Clone, Builder)]
pub struct Redirect {
    #[serde(rename = "_type")]
    _type: String,
    pub redirect: Url,
}

#[derive(Debug, Serialize, Deserialize, Clone, Builder)]
pub struct Release {
    pub version: Version,
    pub channel: String,
    #[serde(default = "Vec::new")]
    pub authors: Vec<String>,
    /// Must be a valid SPDX string
    pub license: Option<String>,
    pub license_url: Option<Url>,
    pub targets: Vec<crate::payload::Target>,
}
