use std::cmp::Ordering;
use std::collections::BTreeSet;
use std::str::FromStr;

use serde::{Deserialize, Serialize};
use typed_builder::TypedBuilder;

#[cfg(feature = "structopt")]
use super::parse_set;

#[derive(Debug, Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[serde(rename_all = "lowercase")]
#[cfg_attr(feature = "poem-openapi", derive(poem_openapi::Enum))]
#[cfg_attr(feature = "poem-openapi", oai(rename = "MacOSRebootSpec", rename_all = "lowercase"))]
pub enum RebootSpec {
    Install,
    Uninstall,
    Update,
}

#[derive(thiserror::Error, Debug)]
#[error("Not a valid string for type")]
pub struct FromStrError;

impl FromStr for RebootSpec {
    type Err = FromStrError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "install" => Ok(RebootSpec::Install),
            "uninstall" => Ok(RebootSpec::Uninstall),
            "update" => Ok(RebootSpec::Update),
            _ => Err(FromStrError),
        }
    }
}

#[derive(
    Debug, Serialize, Deserialize, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, TypedBuilder,
)]
#[cfg_attr(feature = "structopt", derive(structopt::StructOpt))]
#[cfg_attr(feature = "poem-openapi", derive(poem_openapi::Object))]
#[cfg_attr(feature = "poem-openapi", oai(rename = "MacOSPackage"))]
pub struct Package {
    #[builder(default = "MacOSPackage".into(), setter(skip))]
    #[serde(rename = "type")]
    #[cfg_attr(feature = "structopt", structopt(skip))]
    #[cfg_attr(feature = "poem-openapi", oai(rename = "type"))]
    _type: String,

    #[cfg_attr(feature = "structopt", structopt(short, long))]
    pub url: url::Url,

    #[cfg_attr(feature = "structopt", structopt(short, long))]
    pub pkg_id: String,

    #[serde(default, skip_serializing_if = "BTreeSet::is_empty")]
    #[cfg_attr(feature = "structopt", structopt(default_value = "", short, long, parse(try_from_str = parse_set)))]
    #[cfg_attr(feature = "poem-openapi", oai(default))]
    #[builder(default)]
    pub targets: BTreeSet<InstallTarget>,

    #[serde(default, skip_serializing_if = "BTreeSet::is_empty")]
    #[cfg_attr(feature = "structopt", structopt(default_value = "", short, long, parse(try_from_str = parse_set)))]
    #[cfg_attr(feature = "poem-openapi", oai(default))]
    #[builder(default)]
    pub requires_reboot: BTreeSet<RebootSpec>,

    #[cfg_attr(feature = "structopt", structopt(short, long))]
    pub size: u64,

    #[cfg_attr(feature = "structopt", structopt(short, long))]
    pub installed_size: u64,
}

impl super::AsDownloadUrl for Package {
    fn as_download_url(&self) -> &url::Url {
        &self.url
    }
}

#[derive(Debug, Serialize, Deserialize, Clone, Copy, Hash, Eq)]
#[serde(rename_all = "lowercase")]
#[cfg_attr(feature = "poem-openapi", derive(poem_openapi::Enum))]
#[cfg_attr(feature = "poem-openapi", oai(rename_all = "lowercase"))]
pub enum InstallTarget {
    System,
    User,
}

impl std::default::Default for InstallTarget {
    fn default() -> Self {
        InstallTarget::System
    }
}

#[derive(Debug, Clone, Copy, thiserror::Error)]
#[error("Invalid value passed")]
pub struct ParseInstallTargetError;

impl FromStr for InstallTarget {
    type Err = ParseInstallTargetError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "system" => Ok(InstallTarget::System),
            "user" => Ok(InstallTarget::User),
            _ => Err(ParseInstallTargetError {}),
        }
    }
}

impl std::fmt::Display for InstallTarget {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match *self {
            InstallTarget::System => f.write_str("system"),
            InstallTarget::User => f.write_str("user"),
        }
    }
}

impl PartialEq for InstallTarget {
    fn eq(&self, other: &InstallTarget) -> bool {
        match (*self, *other) {
            (InstallTarget::System, InstallTarget::System) => true,
            (InstallTarget::User, InstallTarget::User) => true,
            _ => false,
        }
    }
}

impl PartialOrd for InstallTarget {
    fn partial_cmp(&self, other: &InstallTarget) -> Option<Ordering> {
        Some(self.cmp(&other))
    }
}

impl Ord for InstallTarget {
    fn cmp(&self, other: &InstallTarget) -> Ordering {
        match (*self, *other) {
            (InstallTarget::System, InstallTarget::System) => Ordering::Equal,
            (InstallTarget::User, InstallTarget::User) => Ordering::Equal,
            (InstallTarget::System, InstallTarget::User) => Ordering::Less,
            (InstallTarget::User, InstallTarget::System) => Ordering::Greater,
        }
    }
}
