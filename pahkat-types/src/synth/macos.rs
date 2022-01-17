use serde::{Deserialize, Serialize};
use typed_builder::TypedBuilder;
#[derive(
    Debug, Serialize, Deserialize, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, TypedBuilder,
)]
#[cfg_attr(feature = "poem-openapi", derive(poem_openapi::Object))]
pub struct PackageRef {
    #[serde(rename = "type")]
    #[builder(default = "MacOSPackageRef".into())]
    #[cfg_attr(feature = "poem-openapi", oai(rename = "type"))]
    _type: String,

    pub pkg_id: String,
    pub min_version: Option<String>,
    pub max_version: Option<String>,
    pub min_build: Option<String>,
    pub max_build: Option<String>,
}

#[derive(
    Debug, Serialize, Deserialize, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, TypedBuilder,
)]
#[cfg_attr(feature = "poem-openapi", derive(poem_openapi::Object))]
pub struct PathRef {
    #[serde(rename = "type")]
    #[builder(default = "MacOSPathRef".into())]
    #[cfg_attr(feature = "poem-openapi", oai(rename = "type"))]
    _type: String,

    pub app_paths: Vec<String>,
    pub min_version: Option<String>,
    pub max_version: Option<String>,
    pub min_build: Option<String>,
    pub max_build: Option<String>,
}
