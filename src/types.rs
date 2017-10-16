use std::collections::HashMap;

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
pub struct PackageIndexInstaller {
    pub url: String,
    pub silent_args: String,
    pub guid: String,
    pub size: usize,
    pub installed_size: usize,
    pub signature: PackageIndexInstallerSignature
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PackageIndexInstallerSignature {
    pub public_key: String,
    pub method: String,
    pub hash_algorithm: String,
    pub data: String
}
