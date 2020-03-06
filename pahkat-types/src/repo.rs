use std::collections::BTreeMap;

use derive_builder::Builder;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize, Builder)]
#[serde(rename_all = "camelCase")]
/// The base repository index. All fields may be optionally present except `base_url` and `agent`.
///
/// This struct represents the `index.toml` file at the base of a Pahkat repository.
pub struct Repository {
    #[serde(rename = "_type")]
    pub _type: String,
    pub base_url: String,
    pub agent: RepositoryAgent,
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

/// This struct represents the strings for localising tags and is found in the
/// `strings/` directory at the base of a Pahkat repository.
///
/// The TOML file this struct represents is named after the prefix of the given tag,
/// such that a tag of `category:keyboards` would look up `strings/category.toml`.
pub struct RepositoryLocalisation {
    // TODO
}

#[derive(Clone, Debug, Serialize, Deserialize, Builder)]
pub struct RepositoryRedirect {
    #[serde(rename = "_type")]
    pub _type: String,

    pub redirect: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, Builder)]
#[serde(rename_all = "camelCase")]
pub struct RepositoryAgent {
    name: String,
    version: String,
    url: Option<String>,
}

// impl Default for RepositoryAgent {
//     fn default() -> Self {
//         RepositoryAgent {
//             name: "pahkat".to_string(),
//             version: env!("CARGO_PKG_VERSION").to_owned(),
//             url: Some("https://github.com/divvun/pahkat".to_owned()),
//         }
//     }
// }
