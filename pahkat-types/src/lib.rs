#![allow(deprecated)] // until 1.0

use std::cmp::Ordering;
use std::collections::{BTreeMap, BTreeSet};
use std::str::FromStr;

use derive_builder::Builder;
use serde::{Deserialize, Serialize};

pub use self::repo::{Repository, RepositoryAgent};

pub mod repo;

#[cfg(target_os = "macos")]
pub const OS: &str = "macos";
#[cfg(target_os = "linux")]
pub const OS: &str = "linux";
#[cfg(target_os = "windows")]
pub const OS: &str = "windows";

pub type PackageMap = BTreeMap<String, Package>;
pub type VirtualMap = BTreeMap<String, Virtual>;

#[deprecated(note = "Will be removed in 1.0; no unknown fields will be accepted.")]
fn unknown() -> String {
    "Unknown".into()
}

#[deprecated(note = "Will be removed in 1.0; no unknown fields will be accepted.")]
fn unknown_vec() -> Vec<String> {
    vec![unknown()]
}

#[derive(Debug, Serialize, Deserialize, Clone, Builder)]
#[serde(rename_all = "camelCase")]
pub struct Package {
    #[serde(rename = "@context")]
    pub _context: Option<String>,
    #[serde(rename = "@type")]
    pub _type: Option<String>,
    pub id: String,
    pub name: BTreeMap<String, String>,
    pub description: BTreeMap<String, String>,
    #[serde(default = "unknown_vec")]
    pub authors: Vec<String>,
    #[serde(default = "unknown")]
    pub license: String,
    pub version: String,
    pub category: String,
    pub languages: Vec<String>,
    pub platform: BTreeMap<String, String>,
    #[serde(default = "BTreeMap::new")]
    pub dependencies: BTreeMap<String, String>,
    #[serde(default = "BTreeMap::new")]
    pub virtual_dependencies: BTreeMap<String, String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub installer: Option<Installer>,
}

impl Package {
    pub fn installer(&self) -> Option<&Installer> {
        match &self.installer {
            Some(ref v) => Some(&v),
            None => None,
        }
    }
}

pub trait Downloadable {
    fn url(&self) -> String;
}

#[derive(Debug, Serialize, Deserialize, Clone, Builder)]
#[serde(rename_all = "camelCase")]
pub struct Packages {
    #[serde(rename = "@context")]
    pub _context: Option<String>,
    #[serde(rename = "@type")]
    pub _type: Option<String>,
    #[serde(rename = "@id")]
    pub _id: Option<String>,
    pub base: String,
    pub channel: String,
    #[serde(default = "BTreeMap::new")]
    pub packages: PackageMap,
}

#[derive(Debug, Serialize, Deserialize, Clone, Builder)]
#[serde(rename_all = "camelCase")]
pub struct Virtual {
    #[serde(rename = "@context")]
    pub _context: Option<String>,
    #[serde(rename = "@type")]
    pub _type: Option<String>,
    pub id: String,
    pub name: BTreeMap<String, String>,
    pub version: String,
    #[serde(default = "BTreeMap::new")]
    pub description: BTreeMap<String, String>,
    pub help: BTreeMap<String, String>,
    pub url: Option<String>,
    pub target: VirtualTarget,
}

#[derive(Clone, Debug, Serialize, Deserialize, Builder)]
#[serde(rename_all = "camelCase")]
pub struct Virtuals {
    #[serde(rename = "@context")]
    pub _context: Option<String>,
    #[serde(rename = "@type")]
    pub _type: Option<String>,
    #[serde(rename = "@id")]
    pub _id: Option<String>,
    pub base: String,
    pub channel: String,
    #[serde(default = "BTreeMap::new")]
    pub virtuals: VirtualMap,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(untagged)]
pub enum VirtualTarget {
    WindowsRegistryKey(RegistryKey),
    MacOSPackage(MacOSPackageRef),
    MacOSPath(MacOSPathRef),
}

#[derive(Clone, Debug, Serialize, Deserialize, Builder)]
#[serde(rename_all = "camelCase")]
pub struct MacOSPackageRef {
    #[serde(rename = "@type")]
    pub _type: Option<String>,

    pub pkg_id: String,
    pub min_version: Option<String>,
    pub max_version: Option<String>,
    pub min_build: Option<String>,
    pub max_build: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize, Builder)]
#[serde(rename_all = "camelCase")]
pub struct MacOSPathRef {
    #[serde(rename = "@type")]
    pub _type: Option<String>,

    pub app_paths: Vec<String>,
    pub min_version: Option<String>,
    pub max_version: Option<String>,
    pub min_build: Option<String>,
    pub max_build: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize, Builder)]
#[serde(rename_all = "camelCase")]
pub struct RegistryKey {
    #[serde(rename = "@type")]
    pub _type: Option<String>,

    pub path: String,
    pub name: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Installer {
    Windows(WindowsInstaller),
    MacOS(MacOSInstaller),
    Tarball(TarballInstaller),
}

impl Downloadable for Installer {
    fn url(&self) -> String {
        match *self {
            Installer::Windows(ref v) => v.url.to_owned(),
            Installer::MacOS(ref v) => v.url.to_owned(),
            Installer::Tarball(ref v) => v.url.to_owned(),
        }
    }
}

#[derive(Debug, Clone, Copy, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum InstallTarget {
    System,
    User,
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

#[derive(Clone, Debug, Serialize, Deserialize, Builder)]
#[serde(rename_all = "camelCase")]
pub struct MacOSInstaller {
    #[serde(rename = "@type")]
    pub _type: Option<String>,
    pub url: String,
    pub pkg_id: String,
    #[serde(default)]
    pub targets: BTreeSet<InstallTarget>,
    #[serde(default)]
    pub requires_reboot: bool,
    #[serde(default)]
    pub requires_uninstall_reboot: bool,
    pub size: usize,
    pub installed_size: usize,
    pub signature: Option<InstallerSignature>,
}

#[derive(Clone, Debug, Serialize, Deserialize, Builder)]
#[serde(rename_all = "camelCase")]
pub struct WindowsInstaller {
    #[serde(rename = "@type")]
    pub _type: Option<String>,
    pub url: String,
    #[serde(rename = "type")]
    pub installer_type: Option<String>,
    pub args: Option<String>,
    pub uninstall_args: Option<String>,
    pub product_code: String,
    #[serde(default)]
    pub requires_reboot: bool,
    #[serde(default)]
    pub requires_uninstall_reboot: bool,
    pub size: usize,
    pub installed_size: usize,
    pub signature: Option<InstallerSignature>,
}

#[derive(Clone, Debug, Serialize, Deserialize, Builder)]
#[serde(rename_all = "camelCase")]
pub struct TarballInstaller {
    #[serde(rename = "@type")]
    pub _type: Option<String>,
    pub url: String,
    pub size: usize,
    pub installed_size: usize,
}

#[derive(Clone, Debug, Serialize, Deserialize, Builder)]
#[serde(rename_all = "camelCase")]
pub struct InstallerSignature {
    pub public_key: String,
    pub method: String,
    pub hash_algorithm: String,
    pub data: String,
}
