pub mod version;

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::convert::TryFrom;
use typed_builder::TypedBuilder;
use url::Url;

use crate::LangTagMap;
pub use version::Version;

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[serde(rename_all = "camelCase")]
#[serde(tag = "_type")]
pub enum Package {
    #[serde(rename = "Package")]
    Concrete(Descriptor),
    #[serde(rename = "SyntheticPackage")]
    Synthetic(crate::synth::Descriptor),
    #[serde(rename = "PackageRedirect")]
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

#[derive(
    Debug, Serialize, Deserialize, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, TypedBuilder,
)]
#[serde(rename_all = "camelCase")]
pub struct Index {
    #[serde(default)]
    #[builder(default)]
    pub packages: BTreeMap<String, Package>,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq, Hash, TypedBuilder)]
#[serde(rename_all = "camelCase")]
pub struct Descriptor {
    #[builder(default = "Package".into())]
    _type: String,

    pub id: String,

    #[serde(default)]
    #[builder(default)]
    pub name: LangTagMap<String>,
    #[serde(default)]
    #[builder(default)]
    pub description: LangTagMap<String>,
    #[serde(default)]
    #[builder(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    #[builder(default)]
    pub releases: Vec<Release>,
}

impl PartialOrd for Descriptor {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        self.id.partial_cmp(&other.id)
    }
}

impl Ord for Descriptor {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.id.cmp(&other.id)
    }
}

#[derive(
    Debug, Serialize, Deserialize, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, TypedBuilder,
)]
#[serde(rename_all = "camelCase")]
pub struct Redirect {
    #[builder(default = "PackageRedirect".into())]
    _type: String,

    pub redirect: Url,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq, Hash, TypedBuilder)]
#[serde(rename_all = "camelCase")]
pub struct Release {
    pub version: Version,

    #[builder(default)]
    pub channel: Option<String>,
    #[serde(default)]
    #[builder(default)]
    pub authors: Vec<String>,
    /// Must be a valid SPDX string
    #[builder(default)]
    pub license: Option<String>,
    #[builder(default)]
    pub license_url: Option<Url>,
    #[builder(default)]
    pub targets: Vec<crate::payload::Target>,
}

impl PartialOrd for Release {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        self.version.partial_cmp(&other.version)
    }
}

impl Ord for Release {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        if let Some(v) = self.version.partial_cmp(&other.version) {
            return v;
        }

        self.channel.cmp(&other.channel)
    }
}
