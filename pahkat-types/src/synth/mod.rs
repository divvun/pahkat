pub mod macos;
pub mod windows;

use serde::{Deserialize, Serialize};
use typed_builder::TypedBuilder;

use crate::{DependencyMap, LangTagMap};

#[derive(
    Debug, Serialize, Deserialize, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, TypedBuilder,
)]
#[serde(rename_all = "camelCase")]
pub struct Descriptor {
    #[builder(default = "SyntheticPackage".into())]
    _type: String,

    pub id: String,
    #[serde(default)]
    #[builder(default)]
    pub name: LangTagMap<String>,
    #[serde(default)]
    #[builder(default)]
    pub description: LangTagMap<String>,
    #[serde(default)]
    #[builder(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    #[builder(default)]
    pub releases: Vec<Release>,
}

#[derive(
    Debug, Serialize, Deserialize, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, TypedBuilder,
)]
#[serde(rename_all = "camelCase")]
pub struct Target {
    pub platform: String,
    #[builder(default)]
    pub arch: Option<String>,
    #[serde(default)]
    #[builder(default)]
    pub dependencies: DependencyMap,
    // TODO: have other metadata here for if version not found?
    pub verifier: Verifier,
}

#[derive(
    Debug, Serialize, Deserialize, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, TypedBuilder,
)]
#[serde(rename_all = "camelCase")]
pub struct Release {
    pub version: String,
    pub channel: String,
    #[builder(default)]
    pub targets: Vec<Target>,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[serde(tag = "_type")]
pub enum Verifier {
    WindowsRegistryKey(windows::RegistryKey),
    MacOSPackageRef(macos::PackageRef),
    MacOSPathRef(macos::PathRef),
}
