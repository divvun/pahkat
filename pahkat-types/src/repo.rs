use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Repository {
    #[serde(rename = "@context")]
    pub _context: Option<String>,
    #[serde(rename = "@type")]
    pub _type: Option<String>,
    pub agent: RepositoryAgent,
    pub base: String,
    pub name: BTreeMap<String, String>,
    #[serde(default = "BTreeMap::new")]
    pub description: BTreeMap<String, String>,
    pub primary_filter: String,
    pub default_channel: String,
    pub channels: Vec<String>,
    #[serde(default = "BTreeMap::new")]
    pub categories: BTreeMap<String, BTreeMap<String, String>>,
    #[serde(default = "Vec::new")]
    pub linked_repositories: Vec<String>
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RepositoryAgent {
    #[serde(rename = "@type")]
    pub _type: Option<String>,
    name: String,
    version: String,
    url: Option<String>,
}

impl Default for RepositoryAgent {
    fn default() -> Self {
        RepositoryAgent {
            _type: Some("RepositoryAgent".to_owned()),
            name: "pahkat".to_string(),
            version: env!("CARGO_PKG_VERSION").to_owned(),
            url: Some("https://github.com/divvun/pahkat".to_owned()),
        }
    }
}
