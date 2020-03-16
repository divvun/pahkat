use serde::{Deserialize, Serialize};
use typed_builder::TypedBuilder;
use url::Url;
use std::collections::BTreeMap;

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[serde(tag = "_type")]
pub enum Repository {
    #[serde(rename = "Repository")]
    Index(Index),
    #[serde(rename = "RepositoryRedirect")]
    Redirect(Redirect),
}

#[derive(
    Debug, Serialize, Deserialize, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, TypedBuilder,
)]
#[serde(rename_all = "camelCase")]
/// The base repository index. All fields may be optionally present except `base_url` and `agent`.
///
/// This struct represents the `index.toml` file at the base of a Pahkat repository.
pub struct Index {
    #[serde(rename = "_type")]
    #[builder(default = "Repository".into())]
    _type: String,

    pub base_url: Url,

    #[serde(skip_serializing_if = "Option::is_none")]
    #[builder(default)]
    pub landing_url: Option<Url>,

    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    #[builder(default)]
    pub channels: Vec<String>,

    #[serde(skip_serializing_if = "Option::is_none")]
    #[builder(default)]
    pub default_channel: Option<String>,

    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    #[builder(default)]
    pub linked_repositories: Vec<Url>,

    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    #[builder(default)]
    pub accepted_redirections: Vec<Url>,

    // Tables need to be at the end of the struct
    pub agent: Agent,

    #[serde(default)]
    #[builder(default)]
    pub name: BTreeMap<String, String>,
    
    #[serde(default)]
    #[builder(default)]
    pub description: BTreeMap<String, String>,
}

#[derive(
    Debug, Serialize, Deserialize, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, TypedBuilder,
)]
#[serde(rename_all = "camelCase")]
pub struct Agent {
    pub name: String,
    pub version: String,
    #[builder(default)]
    pub url: Option<Url>,
}

/// This struct represents the strings for localising tags and is found in the
/// `strings/` directory at the base of a Pahkat repository.
///
/// The TOML file this struct represents is named after the prefix of the given tag,
/// such that a tag of `category:keyboards` would look up `strings/category.toml`.
pub struct Localisation {
    // TODO
}

#[derive(
    Debug, Serialize, Deserialize, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, TypedBuilder,
)]
#[serde(rename_all = "camelCase")]
pub struct Redirect {
    #[builder(default = "RepositoryRedirect".into())]
    _type: String,

    pub redirect: url::Url,
}
