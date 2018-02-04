use std::collections::HashMap;

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Repository {
    #[serde(rename = "@context")]
    pub _context: Option<String>,
    #[serde(rename = "@type")]
    pub _type: Option<String>,
    pub agent: Option<RepositoryAgent>,
    pub base: String,
    pub name: HashMap<String, String>,
    pub description: HashMap<String, String>,
    pub primary_filter: String,
    pub channels: Vec<String>
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RepositoryAgent {
    #[serde(rename = "@type")]
    pub _type: Option<String>,
    name: String,
    version: String,
    url: Option<String>
}

impl Default for RepositoryAgent {
    fn default() -> Self {
        RepositoryAgent {
            _type: Some("RepositoryAgent".to_owned()),
            name: "pahkat".to_string(),
            version: env!("CARGO_PKG_VERSION").to_owned(),
            url: Some("https://github.com/divvun/pahkat".to_owned())
        }
    }
}
