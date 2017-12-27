use std::collections::HashMap;

pub mod repo;
pub use repo::*;

#[cfg(target_os = "macos")]
pub const OS: &str = "macos";
#[cfg(target_os = "linux")]
pub const OS: &str = "linux";
#[cfg(target_os = "windows")]
pub const OS: &str = "windows";

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PackageIndex {
    pub id: String,
    pub name: HashMap<String, String>,
    pub description: HashMap<String, String>,
    pub version: String,
    pub category: String,
    pub languages: Vec<String>,
    pub os: HashMap<String, String>,
    #[serde(default = "HashMap::new")]
    pub dependencies: HashMap<String, String>,
    #[serde(default = "HashMap::new")]
    pub virtual_dependencies: HashMap<String, String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub installer: Option<PackageIndexInstaller>
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VirtualIndex {
    pub id: String,
    pub name: HashMap<String, String>,
    #[serde(default = "HashMap::new")]
    pub description: HashMap<String, String>,
    pub version: String,
    pub url: String,
    #[serde(rename = "virtual")]
    pub virtual_: bool,
    pub target: VirtualIndexTarget
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VirtualIndexTarget {
    pub registry_key: Option<VirtualIndexRegistryKey>
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VirtualIndexRegistryKey {
    pub path: String,
    pub name: String
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PackageIndexInstaller {
    pub url: String,
    #[serde(rename = "type")]
    pub type_: Option<String>,
    pub args: Option<String>,
    pub uninstall_args: Option<String>,
    pub product_code: String,
    #[serde(default)]
    pub requires_reboot: bool,
    #[serde(default)]
    pub requires_uninstall_reboot: bool,
    pub size: usize,
    pub installed_size: usize,
    pub signature: Option<PackageIndexInstallerSignature>
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PackageIndexInstallerSignature {
    pub public_key: String,
    pub method: String,
    pub hash_algorithm: String,
    pub data: String
}
