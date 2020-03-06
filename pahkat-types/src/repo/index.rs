use std::collections::BTreeMap;

use derive_builder::Builder;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize, Builder)]
#[serde(rename_all = "camelCase")]
/// The base repository index. All fields may be optionally present except `base_url` and `agent`.
///
/// This struct represents the `index.toml` file at the base of a Pahkat repository.
pub struct Index {
    #[serde(rename = "_type")]
    _type: String,
    
    pub base_url: String,
    pub agent: Agent,
    pub landing_url: Option<String>,
    #[serde(default = "BTreeMap::new")]
    pub name: BTreeMap<String, String>,
    #[serde(default = "BTreeMap::new")]
    pub description: BTreeMap<String, String>,
    #[serde(default = "Vec::new")]
    pub channels: Vec<String>,
    pub default_channel: Option<String>,
    #[serde(default = "Vec::new")]
    pub linked_repositories: Vec<String>,
    #[serde(default = "Vec::new")]
    pub accepted_redirections: Vec<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize, Builder)]
#[serde(rename_all = "camelCase")]
pub struct Agent {
    name: String,
    version: String,
    url: Option<String>,
}
