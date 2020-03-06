use std::cmp::Ordering;
use std::collections::BTreeSet;
use std::str::FromStr;

use derive_builder::Builder;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize, Builder)]
#[serde(rename_all = "camelCase")]
pub struct Package {
    #[serde(rename = "_type")]
    _type: String,

    pub url: url::Url,
    pub pkg_id: String,
    #[serde(default)]
    pub targets: BTreeSet<InstallTarget>,
    #[serde(default)]
    pub requires_reboot: bool,
    #[serde(default)]
    pub requires_uninstall_reboot: bool,
    pub size: usize,
    pub installed_size: usize,
}

impl super::AsDownloadUrl for Package {
    fn as_download_url(&self) -> &url::Url {
        &self.url
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, Eq)]
pub enum InstallTarget {
    System,
    User,
}

impl std::default::Default for InstallTarget {
    fn default() -> Self {
        InstallTarget::System
    }
}

#[derive(Debug, Clone, Copy)]
pub struct ParseInstallTargetError;

impl std::error::Error for ParseInstallTargetError {
    fn description(&self) -> &str {
        "Invalid value passed"
    }
}

impl std::fmt::Display for ParseInstallTargetError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        use std::error::Error;
        write!(f, "{}", self.description())
    }
}

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
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        write!(
            f,
            "{}",
            match *self {
                InstallTarget::System => "system",
                InstallTarget::User => "user",
            }
        )
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
