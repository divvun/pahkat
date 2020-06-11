pub mod version;

use serde::{Deserialize, Serialize};
use std::convert::TryFrom;
use typed_builder::TypedBuilder;
use url::Url;

use crate::LangTagMap;
pub use version::Version;

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[serde(untagged)] // #[serde(tag = "_type")]
pub enum Package {
    #[serde(rename = "Package")]
    Concrete(Descriptor),
    #[serde(rename = "SyntheticPackage")]
    Synthetic(crate::synth::Descriptor),
    #[serde(rename = "PackageRedirect")]
    Redirect(Redirect),
}

impl Package {
    #[inline]
    pub fn id(&self) -> &str {
        match self {
            Package::Concrete(d) => &d.package.id,
            Package::Synthetic(d) => &d.synthetic.id,
            Package::Redirect(d) => &d.redirect.id,
        }
    }
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

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq, Hash, TypedBuilder)]
#[non_exhaustive]
pub struct DescriptorData {
    pub id: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    #[builder(default)]
    pub tags: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq, Hash, TypedBuilder)]
#[non_exhaustive]
pub struct Descriptor {
    // Tables have to come last in TOML
    pub package: DescriptorData,

    #[serde(default)]
    #[builder(default)]
    pub name: LangTagMap<String>,
    #[serde(default)]
    #[builder(default)]
    pub description: LangTagMap<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    #[builder(default)]
    pub release: Vec<Release>,
}

impl PartialOrd for Descriptor {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        self.package.id.partial_cmp(&other.package.id)
    }
}

impl Ord for Descriptor {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.package.id.cmp(&other.package.id)
    }
}

#[derive(
    Debug, Serialize, Deserialize, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, TypedBuilder,
)]
#[non_exhaustive]
pub struct RedirectData {
    pub id: String,
    pub url: Url,
}

#[derive(
    Debug, Serialize, Deserialize, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, TypedBuilder,
)]
#[non_exhaustive]
pub struct Redirect {
    pub redirect: RedirectData,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq, Hash, TypedBuilder)]
#[non_exhaustive]
pub struct Release {
    pub version: Version,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[builder(default)]
    pub channel: Option<String>,

    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    #[builder(default)]
    pub authors: Vec<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[builder(default)]
    /// Must be a valid SPDX string
    pub license: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[builder(default)]
    pub license_url: Option<Url>,

    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    #[builder(default)]
    pub target: Vec<crate::payload::Target>,
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
