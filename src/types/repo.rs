use serde_json;
use std::collections::HashMap;
use std::error::Error;
use std::path::{Path, PathBuf};
use std::fs::File;

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RepositoryMeta {
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
    name: String,
    version: String,
    url: Option<String>
}

impl Default for RepositoryAgent {
    fn default() -> Self {
        RepositoryAgent {
            name: "pahkat".to_string(),
            version: env!("CARGO_PKG_VERSION").to_owned(),
            url: Some("https://github.com/divvun/pahkat".to_owned())
        }
    }
}

// pub struct RepositoryContext {
//     pub index: Option<Repository>,
//     pub path: PathBuf
// }

// pub struct RepoPackagesContext {
//     pub index: Option<RepoPackages>,
//     pub path: PathBuf
// }

// pub struct RepoVirtualsContext {
//     pub index: Option<RepoVirtuals>,
//     pub path: PathBuf
// }

// impl RepositoryContext {
//     fn virtuals(&self) -> RepoVirtualsContext {
//         RepoVirtualsContext::new(&self.path.join("virtuals"))
//     }
// }

// fn repo_virtuals_index_from_file(path: &Path) -> Result<RepoVirtuals, Box<Error>> {
//     let file = File::open(path)?;
//     let index = serde_json::from_reader(file)?;
//     Ok(index)
// }

// impl RepoVirtualsContext {
//     fn new(path: &Path) -> RepoVirtualsContext {
//         let index_path = path.join("index.json");
//         // TODO: fail on errors that are not NotExist
//         let index = repo_virtuals_index_from_file(&index_path).ok();

//         RepoVirtualsContext {
//             index: index,
//             path: path.into()
//         }
//     }
// }
