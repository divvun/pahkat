use std::cmp::Ordering;
use std::str::FromStr;

use serde::{Deserialize, Serialize};
use typed_builder::TypedBuilder;

#[derive(
    Debug, Serialize, Deserialize, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, TypedBuilder,
)]
pub struct Executable {
    #[builder(default = "WindowsExecutable".into())]
    #[serde(rename = "type")]
    _type: String,

    pub url: url::Url,
    pub product_code: String,
    pub size: u64,
    pub installed_size: u64,

    /// The type of installer (msi, nsis, etc)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[builder(default)]
    pub kind: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[builder(default)]
    pub args: Option<String>,

    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[builder(default)]
    pub uninstall_args: Option<String>,

    #[serde(default, skip_serializing_if = "crate::is_false")]
    #[builder(default)]
    pub requires_reboot: bool,

    #[serde(default, skip_serializing_if = "crate::is_false")]
    #[builder(default)]
    pub requires_uninstall_reboot: bool,
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
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
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
