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
    pub installer: Option<WindowsInstaller>
}


#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Packages {
    #[serde(rename = "@type")]
    pub _type: Option<String>,
    #[serde(default = "HashMap::new")]
    pub packages: PackageMap
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Virtual {
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
   #[serde(rename = "@type")]
    pub _type: Option<String>,
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
    pub signature: Option<WindowsInstallerSignature>
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WindowsInstallerSignature {
    pub public_key: String,
    pub method: String,
    pub hash_algorithm: String,
    pub data: String
}