pub mod index;

use crate::LangTagMap;
use derive_builder::Builder;
use serde::{Deserialize, Serialize};

pub use index::Index;

#[derive(Debug, Serialize, Deserialize, Clone, Builder)]
#[serde(rename_all = "camelCase")]
pub struct Package {
    #[serde(rename = "_type")]
    _type: String,
    pub id: String,

    #[serde(default = "LangTagMap::new")]
    pub name: LangTagMap<String>,
    #[serde(default = "LangTagMap::new")]
    pub description: LangTagMap<String>,
    #[serde(default = "Vec::new")]
    pub tags: Vec<String>,
    #[serde(default = "Vec::new")]
    pub versions: Vec<Version>,
}

#[derive(Debug, Serialize, Deserialize, Clone, Builder)]
pub struct Redirect {
    #[serde(rename = "_type")]
    _type: String,
    pub redirect: String,
}

#[derive(Debug, Serialize, Deserialize, Clone, Builder)]
pub struct Version {
    pub version: String,
    pub channel: String,
    #[serde(default = "Vec::new")]
    pub authors: Vec<String>,
    /// Must be a valid SPDX string
    pub license: Option<String>,
    pub license_url: Option<url::Url>,
    pub targets: Vec<crate::payload::Target>,
}
