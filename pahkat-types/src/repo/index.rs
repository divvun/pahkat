use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use typed_builder::TypedBuilder;
use url::Url;

#[derive(
    Debug, Serialize, Deserialize, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, TypedBuilder,
)]
#[serde(rename_all = "camelCase")]
/// The base repository index. All fields may be optionally present except `base_url` and `agent`.
///
/// This struct represents the `index.toml` file at the base of a Pahkat repository.
pub struct Index {
    #[builder(default = "Repository".into())]
    _type: String,

    pub base_url: Url,
    pub agent: Agent,
    #[builder(default)]
    pub landing_url: Option<Url>,
    #[serde(default)]
    #[builder(default)]
    pub name: BTreeMap<String, String>,
    #[serde(default)]
    #[builder(default)]
    pub description: BTreeMap<String, String>,
    #[serde(default)]
    #[builder(default)]
    pub channels: Vec<String>,
    #[builder(default)]
    pub default_channel: Option<String>,
    #[serde(default)]
    #[builder(default)]
    pub linked_repositories: Vec<Url>,
    #[serde(default)]
    #[builder(default)]
    pub accepted_redirections: Vec<Url>,
}

#[derive(
    Debug, Serialize, Deserialize, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, TypedBuilder,
)]
#[serde(rename_all = "camelCase")]
pub struct Agent {
    name: String,
    version: String,
    #[builder(default)]
    url: Option<Url>,
}
