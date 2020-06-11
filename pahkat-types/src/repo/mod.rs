mod url;

pub use self::url::{RepoUrl, RepoUrlError};

use ::url::Url;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use typed_builder::TypedBuilder;

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[serde(untagged)]
#[non_exhaustive]
pub enum Repository {
    Index(Index),
    Redirect(Redirect),
}

#[derive(
    Debug, Serialize, Deserialize, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, TypedBuilder,
)]
#[non_exhaustive]
pub struct RepositoryData {
    pub url: RepoUrl,

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
    pub linked_repositories: Vec<RepoUrl>,

    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    #[builder(default)]
    pub accepted_redirections: Vec<RepoUrl>,
}

#[derive(
    Debug, Serialize, Deserialize, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, TypedBuilder,
)]
#[non_exhaustive]
/// The base repository index. All fields may be optionally present except `url` and `agent`.
///
/// This struct represents the `index.toml` file at the base of a Pahkat repository.
pub struct Index {
    pub repository: RepositoryData,

    #[serde(default)]
    #[builder(default)]
    pub name: BTreeMap<String, String>,

    #[serde(default)]
    #[builder(default)]
    pub description: BTreeMap<String, String>,

    pub agent: Agent,
}

#[derive(
    Debug, Serialize, Deserialize, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, TypedBuilder,
)]
#[non_exhaustive]
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
#[non_exhaustive]
pub struct RedirectData {
    pub url: RepoUrl,
}

#[derive(
    Debug, Serialize, Deserialize, Clone, PartialEq, Eq, Hash, PartialOrd, Ord, TypedBuilder,
)]
#[non_exhaustive]
pub struct Redirect {
    pub redirect: RedirectData,
}
