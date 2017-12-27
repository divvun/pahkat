use serde_json;
use std::collections::HashMap;
use std::error::Error;
use std::path::{Path, PathBuf};
use std::fs::File;

pub type RepoPackagesIndex = HashMap<String, String>;
pub type RepoVirtualsIndex = HashMap<String, Vec<String>>; 

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RepoIndex {
    #[serde(rename = "@type")]
    pub _type: String,
    pub agent: Option<RepoAgent>,
    pub base: String,
    pub name: HashMap<String, String>,
    pub description: HashMap<String, String>,
    pub primary_filter: String,
    pub channels: Vec<String>
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RepoAgent {
    name: String,
    version: String,
    url: Option<String>
}

impl Default for RepoAgent {
    fn default() -> Self {
        RepoAgent {
            name: "bahkat".to_string(),
            version: crate_version!().to_owned(),
            url: Some("https://github.com/divvun/bahkat".to_owned())
        }
    }
}

pub struct RepoIndexContext {
    pub index: Option<RepoIndex>,
    pub path: PathBuf
}

pub struct RepoPackagesIndexContext {
    pub index: Option<RepoPackagesIndex>,
    pub path: PathBuf
}

pub struct RepoVirtualsIndexContext {
    pub index: Option<RepoVirtualsIndex>,
    pub path: PathBuf
}

impl RepoIndexContext {
    fn virtuals(&self) -> RepoVirtualsIndexContext {
        RepoVirtualsIndexContext::new(&self.path.join("virtuals"))
    }
}

fn repo_virtuals_index_from_file(path: &Path) -> Result<RepoVirtualsIndex, Box<Error>> {
    let file = File::open(path)?;
    let index = serde_json::from_reader(file)?;
    Ok(index)
}

impl RepoVirtualsIndexContext {
    fn new(path: &Path) -> RepoVirtualsIndexContext {
        let index_path = path.join("index.json");
        // TODO: fail on errors that are not NotExist
        let index = repo_virtuals_index_from_file(&index_path).ok();

        RepoVirtualsIndexContext {
            index: index,
            path: path.into()
        }
    }
}
