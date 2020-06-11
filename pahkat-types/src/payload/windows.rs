use std::cmp::Ordering;
use std::collections::BTreeSet;
use std::str::FromStr;

use serde::{Deserialize, Serialize};
use typed_builder::TypedBuilder;

#[cfg(feature = "structopt")]
use super::parse_set;

#[derive(Debug, Serialize, Deserialize, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[serde(rename_all = "lowercase")]
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

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[serde(transparent)]
#[repr(transparent)]
struct PayloadType(String);

impl Default for PayloadType {
    fn default() -> Self {
        PayloadType("WindowsExecutable".into())
    }
}

#[derive(
    Debug, Serialize, Deserialize, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, TypedBuilder,
)]
#[cfg_attr(feature = "structopt", derive(structopt::StructOpt))]
pub struct Executable {
    #[builder(default, setter(skip))]
    #[serde(rename = "type")]
    #[cfg_attr(feature = "structopt", structopt(skip))]
    _type: PayloadType,

    #[cfg_attr(feature = "structopt", structopt(short, long))]
    pub url: url::Url,
    #[cfg_attr(feature = "structopt", structopt(short, long))]
    pub product_code: String,
    #[cfg_attr(feature = "structopt", structopt(short, long))]
    pub size: u64,
    #[cfg_attr(feature = "structopt", structopt(short, long))]
    pub installed_size: u64,

    /// The type of installer (msi, nsis, etc)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[builder(default)]
    #[cfg_attr(feature = "structopt", structopt(short, long))]
    pub kind: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[builder(default)]
    #[cfg_attr(feature = "structopt", structopt(short = "I", long))]
    pub args: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[builder(default)]
    #[cfg_attr(feature = "structopt", structopt(short = "U", long))]
    pub uninstall_args: Option<String>,

    #[serde(default, skip_serializing_if = "BTreeSet::is_empty")]
    #[cfg_attr(feature = "structopt", structopt(default_value = "", short, long, parse(try_from_str = parse_set)))]
    #[builder(default)]
    pub requires_reboot: BTreeSet<RebootSpec>,
}

impl super::AsDownloadUrl for Executable {
    fn as_download_url(&self) -> &url::Url {
        &self.url
    }
}

#[derive(Debug, Serialize, Deserialize, Clone, Copy, Eq, Hash)]
#[serde(rename_all = "lowercase")]
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
