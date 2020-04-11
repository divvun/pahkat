use std::path::Path;

use serde::{Deserialize, Serialize};
use url::Url;

use crate::pahkat_fbs;

// fn last_modified_cache(repo_cache_dir: &Path) -> SystemTime {
//     match std::fs::metadata(repo_cache_dir.join("cache.json")) {
//         Ok(v) => match v.modified() {
//             Ok(v) => v,
//             Err(_) => std::time::SystemTime::UNIX_EPOCH,
//         },
//         Err(_) => std::time::SystemTime::UNIX_EPOCH,
//     }
// }

#[derive(Debug, thiserror::Error)]
pub enum RepoDownloadError {
    #[error("Error while processing HTTP request")]
    ReqwestError(#[from] reqwest::Error),

    #[error("Error parsing TOML index")]
    TomlError(#[from] toml::de::Error),

    #[error("I/O error")]
    IoError(#[from] std::io::Error),
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct LoadedRepositoryMeta {
    pub channel: Option<String>,
    pub hash_id: String,
    // TODO: last update
}

// rental::rental! {
//     mod rent {
//         #[rental(debug)]
//         pub struct RentedPackages {
//             head: Box<[u8]>,
//             iref: pahkat_fbs::Packages<'head>
//         }
//     }
// }

#[derive(Debug)]
pub struct LoadedRepository {
    info: pahkat_types::repo::Index,
    packages: Box<[u8]>, //rent::RentedPackages,
    // strings: pahkat_types::strings::
    meta: LoadedRepositoryMeta,
}

impl LoadedRepository {
    pub fn from_cache_or_url(
        url: &Url,
        cache_dir: &Path,
    ) -> Result<LoadedRepository, RepoDownloadError> {
        Self::from_url(url)
    }

    fn from_url(url: &Url) -> Result<LoadedRepository, RepoDownloadError> {
        let client = reqwest::blocking::Client::new();

        let info = client.get(&format!("{}/index.toml", url)).send()?.text()?;
        let info: pahkat_types::repo::Index = toml::from_str(&info)?;

        let packages = client
            .get(&format!("{}/packages/index.bin", url))
            .send()?
            .bytes()?
            .to_vec()
            .into_boxed_slice();
        // let packages = rent::RentedPackages::new(packages, |p| pahkat_fbs::get_root_as_packages(&packages));

        // let hash_id = Repository::path_hash(url, &channel);

        // let meta_res = client
        //     .get(&format!("{}/index.json", url))
        //     .send()
        //     .map_err(|e| RepoDownloadError::ReqwestError(e))?;
        // let meta_text = meta_res
        //     .text()
        //     .map_err(|e| RepoDownloadError::ReqwestError(e))?;
        // let meta: RepositoryMeta =
        //     serde_json::from_str(&meta_text).map_err(|e| RepoDownloadError::JsonError(e))?;

        // let index_json_path = if meta.default_channel == channel {
        //     "index.json".into()
        // } else {
        //     format!("index.{}.json", &channel)
        // };

        // let pkg_res = client
        //     .get(&format!("{}/packages/{}", url, index_json_path))
        //     .send()
        //     .map_err(|e| RepoDownloadError::ReqwestError(e))?;
        // let pkg_text = pkg_res
        //     .text()
        //     .map_err(|e| RepoDownloadError::ReqwestError(e))?;
        // let packages: Packages =
        //     serde_json::from_str(&pkg_text).map_err(|e| RepoDownloadError::JsonError(e))?;

        // let virt_res = client
        //     .get(&format!("{}/virtuals/{}", url, index_json_path))
        //     .send()
        //     .map_err(|e| RepoDownloadError::ReqwestError(e))?;
        // let virt_text = virt_res
        //     .text()
        //     .map_err(|e| RepoDownloadError::ReqwestError(e))?;
        // let virtuals: Virtuals =
        //     serde_json::from_str(&virt_text).map_err(|e| RepoDownloadError::JsonError(e))?;

        let repo = LoadedRepository {
            info,
            packages,
            meta: LoadedRepositoryMeta {
                channel: None,
                hash_id: "".into(),
            },
        };

        Ok(repo)
    }

    pub fn info(&self) -> &pahkat_types::repo::Index {
        &self.info
    }

    pub fn packages<'a>(&'a self) -> pahkat_fbs::Packages<&'a [u8]> {
        pahkat_fbs::Packages::get_root(&*self.packages).expect("packages must always exist")
    }

    pub fn meta(&self) -> &LoadedRepositoryMeta {
        &self.meta
    }
}

// #[derive(Debug, Serialize, Deserialize, Clone)]
// // pub struct Repository {
//     meta: RepositoryMeta,
//     packages: Packages,
//     virtuals: Virtuals,
// }

// impl Repository {
//     // pub fn path_hash(url: &Url, channel: &str) -> String {
//     //     let mut sha = Sha256::new();
//     //     sha.input(&format!("{}#{}", &url, &channel));
//     //     format!("{:x}", sha.result())
//     // }

//     pub fn from_cache_or_url(
//         url: &Url,
//         channel: String,
//         cache_dir: &Path,
//     ) -> Result<Repository, RepoDownloadError> {
//         log::debug!("{}, {}, {:?}", url, &channel, cache_dir);
//         let hash_id = Repository::path_hash(url, &channel);

//         let repo_cache_dir = cache_dir.join(&hash_id);

//         if !repo_cache_dir.exists() {
//             log::debug!("Cache does not exist, creating");
//             let repo = Repository::from_url(url, channel)?;
//             repo.save_to_cache(cache_dir)
//                 .map_err(|e| RepoDownloadError::IoError(e))?;
//             log::debug!("Save repo");
//             return Ok(repo);
//         }

//         // Check cache age
//         let ts = last_modified_cache(&repo_cache_dir);

//         // 5 minutes cache check
//         let is_cache_valid = match SystemTime::now().duration_since(ts) {
//             Ok(v) => v.as_secs() < 300,
//             Err(_) => false,
//         };

//         if is_cache_valid {
//             log::debug!("Loading from cache");
//             match Repository::from_directory(cache_dir, hash_id) {
//                 Ok(v) => return Ok(v),
//                 Err(_) => {} // fallthrough
//             }
//         }

//         log::debug!("loading from web");
//         let repo = Repository::from_url(url, channel)?;
//         repo.save_to_cache(cache_dir)
//             .map_err(|e| RepoDownloadError::IoError(e))?;
//         Ok(repo)
//     }

//     fn from_directory(cache_dir: &Path, hash_id: String) -> Result<Repository, RepoDownloadError> {
//         let cache_file = File::open(cache_dir.join(&hash_id).join("cache.json"))
//             .map_err(|e| RepoDownloadError::IoError(e))?;

//         let repo: Repository =
//             serde_json::from_reader(cache_file).map_err(|e| RepoDownloadError::JsonError(e))?;

//         Ok(repo)
//     }

//     pub fn from_url(url: &Url, channel: String) -> Result<Repository, RepoDownloadError> {
//         let client = reqwest::blocking::Client::new();
//         let hash_id = Repository::path_hash(url, &channel);

//         let meta_res = client
//             .get(&format!("{}/index.json", url))
//             .send()
//             .map_err(|e| RepoDownloadError::ReqwestError(e))?;
//         let meta_text = meta_res
//             .text()
//             .map_err(|e| RepoDownloadError::ReqwestError(e))?;
//         let meta: RepositoryMeta =
//             serde_json::from_str(&meta_text).map_err(|e| RepoDownloadError::JsonError(e))?;

//         let index_json_path = if meta.default_channel == channel {
//             "index.json".into()
//         } else {
//             format!("index.{}.json", &channel)
//         };

//         let pkg_res = client
//             .get(&format!("{}/packages/{}", url, index_json_path))
//             .send()
//             .map_err(|e| RepoDownloadError::ReqwestError(e))?;
//         let pkg_text = pkg_res
//             .text()
//             .map_err(|e| RepoDownloadError::ReqwestError(e))?;
//         let packages: Packages =
//             serde_json::from_str(&pkg_text).map_err(|e| RepoDownloadError::JsonError(e))?;

//         let virt_res = client
//             .get(&format!("{}/virtuals/{}", url, index_json_path))
//             .send()
//             .map_err(|e| RepoDownloadError::ReqwestError(e))?;
//         let virt_text = virt_res
//             .text()
//             .map_err(|e| RepoDownloadError::ReqwestError(e))?;
//         let virtuals: Virtuals =
//             serde_json::from_str(&virt_text).map_err(|e| RepoDownloadError::JsonError(e))?;

//         let repo = Repository {
//             meta,
//             packages,
//             virtuals,
//             channel,
//             hash_id,
//         };

//         Ok(repo)
//     }

//     pub fn clear_cache(
//         url: &Url,
//         channel: String,
//         cache_dir: &Path,
//     ) -> Result<(), std::io::Error> {
//         let hash_id = Repository::path_hash(url, &channel);
//         let repo_cache_dir = cache_dir.join(&hash_id);
//         fs::remove_dir_all(&repo_cache_dir)
//     }

//     pub fn save_to_cache(&self, cache_dir: &Path) -> Result<(), std::io::Error> {
//         let hash_cache_dir = cache_dir.join(&self.hash_id);
//         fs::create_dir_all(&hash_cache_dir)?;
//         let file = File::create(hash_cache_dir.join("cache.json"))?;
//         serde_json::to_writer(file, self).expect("repository is always valid JSON");
//         Ok(())
//     }

//     pub fn meta(&self) -> &RepositoryMeta {
//         &self.meta
//     }

//     pub fn channel(&self) -> &str {
//         &self.channel
//     }

//     pub fn package(&self, key: &str) -> Option<&Package> {
//         let map = &self.packages.packages;
//         map.get(key)
//     }

//     pub fn packages(&self) -> &PackageMap {
//         &self.packages.packages
//     }

//     pub fn virtuals(&self) -> &VirtualMap {
//         &self.virtuals.virtuals
//     }
// }
