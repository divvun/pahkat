pub mod macos;
pub mod windows;

use derive_builder::Builder;
use serde::{Deserialize, Serialize};

use crate::{DependencyMap, LangTagMap};

#[derive(Debug, Serialize, Deserialize, Clone, Builder)]
#[serde(rename_all = "camelCase")]
pub struct Package {
    #[serde(rename = "_type")]
    /// Always has value "SyntheticPackage"
    _type: String,

    pub id: String,
    #[serde(default = "LangTagMap::new")]
    pub name: LangTagMap<String>,
    #[serde(default = "LangTagMap::new")]
    pub description: LangTagMap<String>,

    pub url: Option<String>,
    pub versions: Vec<Version>,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct Target {
    pub platform: String,
    pub arch: Option<String>,
    #[serde(default = "DependencyMap::new")]
    pub dependencies: DependencyMap,
    pub verifier: Verifier,
}

#[derive(Debug, Serialize, Deserialize, Clone, Builder)]
#[serde(rename_all = "camelCase")]
pub struct Version {
    pub version: String,
    pub channel: String,
    pub targets: Vec<Target>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Verifier {
    WindowsRegistryKey(windows::RegistryKey),
    MacOSPackage(macos::PackageRef),
    MacOSPath(macos::PathRef),
}
