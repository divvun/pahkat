use derive_builder::Builder;
use serde::{Deserialize, Serialize};
#[derive(Clone, Debug, Serialize, Deserialize, Builder)]
#[serde(rename_all = "camelCase")]
pub struct PackageRef {
    #[serde(rename = "_type")]
    _type: String,

    pub pkg_id: String,
    pub min_version: Option<String>,
    pub max_version: Option<String>,
    pub min_build: Option<String>,
    pub max_build: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize, Builder)]
#[serde(rename_all = "camelCase")]
pub struct PathRef {
    #[serde(rename = "_type")]
    _type: String,

    pub app_paths: Vec<String>,
    pub min_version: Option<String>,
    pub max_version: Option<String>,
    pub min_build: Option<String>,
    pub max_build: Option<String>,
}
