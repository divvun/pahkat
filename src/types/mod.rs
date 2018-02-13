#![allow(dead_code)]

use std::collections::HashMap;

pub mod repo;
pub use self::repo::*;

#[cfg(target_os = "macos")]
pub const OS: &str = "macos";
#[cfg(target_os = "linux")]
pub const OS: &str = "linux";
#[cfg(target_os = "windows")]
pub const OS: &str = "windows";

pub type PackageMap = HashMap<String, Package>;
pub type VirtualRefMap = HashMap<String, Vec<String>>;

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Package {
    #[serde(rename = "@context")]
    pub _context: Option<String>,
    #[serde(rename = "@type")]
    pub _type: Option<String>,
    pub id: String,
    pub name: HashMap<String, String>,
    pub description: HashMap<String, String>,
    pub version: String,
    pub category: String,
    pub languages: Vec<String>,
    pub platform: HashMap<String, String>,
    #[serde(default = "HashMap::new")]
    pub dependencies: HashMap<String, String>,
    #[serde(default = "HashMap::new")]
    pub virtual_dependencies: HashMap<String, String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub installer: Option<Installer>
}

impl Package {
    pub fn installer(&self) -> Option<&Installer> {
        match &self.installer {
            &Some(ref v) => Some(&v),
            &None => None
        }
    }
}

pub trait Downloadable {
    fn url(&self) -> String;
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Packages {
    #[serde(rename = "@context")]
    pub _context: Option<String>,
    #[serde(rename = "@type")]
    pub _type: Option<String>,
    #[serde(rename = "@id")]
    pub _id: Option<String>,
    pub base: String,
    #[serde(default = "HashMap::new")]
    pub packages: PackageMap
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Virtual {
    #[serde(rename = "@context")]
    pub _context: Option<String>,
    #[serde(rename = "@type")]
    pub _type: Option<String>,
    pub id: String,
    pub name: HashMap<String, String>,
    #[serde(default = "HashMap::new")]
    pub description: HashMap<String, String>,
    pub version: String,
    pub url: String,
    #[serde(rename = "virtual")]
    pub virtual_: bool,
    pub target: VirtualTarget
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Virtuals {
    #[serde(rename = "@context")]
    pub _context: Option<String>,
    #[serde(rename = "@type")]
    pub _type: Option<String>,
    #[serde(rename = "@id")]
    pub _id: Option<String>,
    pub base: String,
    #[serde(default = "HashMap::new")]
    pub virtuals: VirtualRefMap
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VirtualTarget {
    pub registry_key: Option<RegistryKey>
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RegistryKey {
    pub path: String,
    pub name: String
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Installer {
    Windows(WindowsInstaller),
    Tarball(TarballInstaller),
    MacOSPackage(MacOSPackageInstaller),
    MacOSBundle(MacOSBundleInstaller)
}

impl Downloadable for Installer {
    fn url(&self) -> String {
        match *self {
            Installer::Windows(ref v) => v.url.to_owned(),
            Installer::Tarball(ref v) => v.url.to_owned(),
            Installer::MacOSPackage(ref v) => v.url.to_owned(),
            Installer::MacOSBundle(ref v) => v.url.to_owned()
        }
    }
}

/// This type is for .bundle files which include an Info.plist for versioning purposes
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MacOSBundleInstaller {
    #[serde(rename = "@type")]
    pub _type: Option<String>,
    pub url: String,
    pub install_path: String,
    #[serde(default)]
    pub requires_reboot: bool,
    #[serde(default)]
    pub requires_uninstall_reboot: bool,
    pub size: usize,
    pub installed_size: usize,
    pub signature: Option<InstallerSignature>
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MacOSPackageInstaller {
    #[serde(rename = "@type")]
    pub _type: Option<String>,
    pub url: String,
    #[serde(default)]
    pub requires_reboot: bool,
    #[serde(default)]
    pub requires_uninstall_reboot: bool,
    pub size: usize,
    pub installed_size: usize,
    pub signature: Option<InstallerSignature>
}

#[derive(Debug, Serialize, Deserialize)]
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
    pub signature: Option<InstallerSignature>
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TarballInstaller {
    #[serde(rename = "@type")]
    pub _type: Option<String>,
    pub url: String,
    pub size: usize,
    pub installed_size: usize
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InstallerSignature {
    pub public_key: String,
    pub method: String,
    pub hash_algorithm: String,
    pub data: String
}
