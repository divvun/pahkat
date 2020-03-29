use serde::{Deserialize, Serialize};
use typed_builder::TypedBuilder;
#[derive(
    Debug, Serialize, Deserialize, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, TypedBuilder,
)]
pub struct PackageRef {
    #[builder(default = "MacOSPackageRef".into())]
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
pub struct PathRef {
    #[builder(default = "MacOSPathRef".into())]
    _type: String,

    pub app_paths: Vec<String>,
    pub min_version: Option<String>,
    pub max_version: Option<String>,
    pub min_build: Option<String>,
    pub max_build: Option<String>,
}
