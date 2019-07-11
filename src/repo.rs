use crypto::digest::Digest;
use crypto::sha2::Sha256;
use pahkat_types::{
    Package, PackageMap, Packages, Repository as RepositoryMeta, VirtualMap, Virtuals,
};
use serde_derive::Serialize;
use std::fs::{self, File};
use std::path::Path;
use std::time::SystemTime;
use url::Url;

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Repository {
    meta: RepositoryMeta,
    packages: Packages,
    virtuals: Virtuals,
    channel: String,
    hash_id: String,
}

fn last_modified_cache(repo_cache_path: &Path) -> SystemTime {
    match std::fs::metadata(repo_cache_path.join("cache.json")) {
        Ok(v) => match v.modified() {
            Ok(v) => v,
            Err(_) => std::time::SystemTime::UNIX_EPOCH,
        },
        Err(_) => std::time::SystemTime::UNIX_EPOCH,
    }
}

use crate::{AbsolutePackageKey, RepoRecord};
use hashbrown::HashMap;
use std::sync::{Arc, RwLock};

pub(crate) fn download_path(config: &StoreConfig, url: &str) -> std::path::PathBuf {
    let mut sha = Sha256::new();
    sha.input_str(url);
    let hash_id = sha.result_str();
    let part1 = &hash_id[0..2];
    let part2 = &hash_id[2..4];
    let part3 = &hash_id[4..];

    config
        .package_cache_path()
        .join(part1)
        .join(part2)
        .join(part3)
}

pub(crate) fn resolve_package(
    package_key: &AbsolutePackageKey,
    repos: &Arc<RwLock<HashMap<RepoRecord, Repository>>>,
) -> Option<Package> {
    repos
        .read()
        .unwrap()
        .get(&RepoRecord {
            url: package_key.url.clone(),
            channel: package_key.channel.clone(),
        })
        .and_then(|r| {
            println!("Got repo: {:?}", r);
            for k in r.packages().keys() {
                println!("Pkg id: {}, {}", &k, k == &package_key.id);
            }

            println!("My pkg id: {}", &package_key.id);
            let pkg = match r.packages().get(&package_key.id) {
                Some(x) => Some(x.to_owned()),
                None => None,
            };
            println!("Found pkg: {:?}", &pkg);
            pkg
        })
}

impl Repository {
    pub fn path_hash(url: &Url, channel: &str) -> String {
        let mut sha = Sha256::new();
        sha.input_str(&format!("{}#{}", &url, &channel));
        sha.result_str()
    }

    pub fn from_cache_or_url(
        url: &Url,
        channel: String,
        cache_path: &Path,
    ) -> Result<Repository, RepoDownloadError> {
        println!("{}, {}, {:?}", url, &channel, cache_path);
        let hash_id = Repository::path_hash(url, &channel);

        let repo_cache_path = cache_path.join(&hash_id);

        if !repo_cache_path.exists() {
            println!("Cache does not exist, creating");
            let repo = Repository::from_url(url, channel)?;
            repo.save_to_cache(cache_path)
                .map_err(|e| RepoDownloadError::IoError(e))?;
            println!("Save repo");
            return Ok(repo);
        }

        // Check cache age
        let ts = last_modified_cache(&repo_cache_path);

        // 5 minutes cache check
        let is_cache_valid = match SystemTime::now().duration_since(ts) {
            Ok(v) => v.as_secs() < 300,
            Err(_) => false,
        };

        if is_cache_valid {
            println!("Loading from cache");
            match Repository::from_directory(cache_path, hash_id) {
                Ok(v) => return Ok(v),
                Err(_) => {} // fallthrough
            }
        }

        println!("loading from web");
        let repo = Repository::from_url(url, channel)?;
        repo.save_to_cache(cache_path)
            .map_err(|e| RepoDownloadError::IoError(e))?;
        Ok(repo)
    }

    fn from_directory(cache_path: &Path, hash_id: String) -> Result<Repository, RepoDownloadError> {
        let cache_file = File::open(cache_path.join(&hash_id).join("cache.json"))
            .map_err(|e| RepoDownloadError::IoError(e))?;

        let repo: Repository =
            serde_json::from_reader(cache_file).map_err(|e| RepoDownloadError::JsonError(e))?;

        Ok(repo)
    }

    pub fn from_url(url: &Url, channel: String) -> Result<Repository, RepoDownloadError> {
        let client = reqwest::Client::new();
        let hash_id = Repository::path_hash(url, &channel);

        let mut meta_res = client
            .get(&format!("{}/index.json", url))
            .send()
            .map_err(|e| RepoDownloadError::ReqwestError(e))?;
        let meta_text = meta_res
            .text()
            .map_err(|e| RepoDownloadError::ReqwestError(e))?;
        let meta: RepositoryMeta =
            serde_json::from_str(&meta_text).map_err(|e| RepoDownloadError::JsonError(e))?;

        let index_json_path = if meta.default_channel == channel {
            "index.json".into()
        } else {
            format!("index.{}.json", &channel)
        };

        let mut pkg_res = client
            .get(&format!("{}/packages/{}", url, index_json_path))
            .send()
            .map_err(|e| RepoDownloadError::ReqwestError(e))?;
        let pkg_text = pkg_res
            .text()
            .map_err(|e| RepoDownloadError::ReqwestError(e))?;
        let packages: Packages =
            serde_json::from_str(&pkg_text).map_err(|e| RepoDownloadError::JsonError(e))?;

        let mut virt_res = client
            .get(&format!("{}/virtuals/{}", url, index_json_path))
            .send()
            .map_err(|e| RepoDownloadError::ReqwestError(e))?;
        let virt_text = virt_res
            .text()
            .map_err(|e| RepoDownloadError::ReqwestError(e))?;
        let virtuals: Virtuals =
            serde_json::from_str(&virt_text).map_err(|e| RepoDownloadError::JsonError(e))?;

        let repo = Repository {
            meta,
            packages,
            virtuals,
            channel,
            hash_id,
        };

        Ok(repo)
    }

    pub fn clear_cache(
        url: &Url,
        channel: String,
        cache_path: &Path,
    ) -> Result<(), std::io::Error> {
        let hash_id = Repository::path_hash(url, &channel);
        let repo_cache_path = cache_path.join(&hash_id);
        fs::remove_dir_all(&repo_cache_path)
    }

    pub fn save_to_cache(&self, cache_path: &Path) -> Result<(), std::io::Error> {
        let hash_cache_path = cache_path.join(&self.hash_id);
        fs::create_dir_all(&hash_cache_path)?;
        let file = File::create(hash_cache_path.join("cache.json"))?;
        serde_json::to_writer(file, self).expect("repository is always valid JSON");
        Ok(())
    }

    pub fn meta(&self) -> &RepositoryMeta {
        &self.meta
    }

    pub fn package(&self, key: &str) -> Option<&Package> {
        let map = &self.packages.packages;
        map.get(key)
    }

    pub fn packages(&self) -> &PackageMap {
        &self.packages.packages
    }

    pub fn virtuals(&self) -> &VirtualMap {
        &self.virtuals.virtuals
    }
}

#[derive(Debug)]
pub enum RepoDownloadError {
    ReqwestError(reqwest::Error),
    JsonError(serde_json::Error),
    IoError(std::io::Error),
}

impl std::fmt::Display for RepoDownloadError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match *self {
            RepoDownloadError::ReqwestError(ref e) => e.fmt(f),
            RepoDownloadError::JsonError(ref e) => e.fmt(f),
            RepoDownloadError::IoError(ref e) => e.fmt(f),
        }
    }
}

use crate::StoreConfig;

pub(crate) fn refresh_repos(config: &StoreConfig) -> HashMap<RepoRecord, Repository> {
    let mut repos = HashMap::new();

    for record in config.repos().iter() {
        recurse_repo(record, &mut repos, &config.repo_cache_path());
    }

    repos
}

fn recurse_linked_repos(
    url: &str,
    channel: String,
    repos: &mut HashMap<RepoRecord, Repository>,
    cache_path: &Path,
) {
    let url = match url::Url::parse(url) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("{:?}", e);
            return;
        }
    };

    let record = RepoRecord { url, channel };

    recurse_repo(&record, repos, cache_path);
}

fn recurse_repo(
    record: &RepoRecord,
    repos: &mut HashMap<RepoRecord, Repository>,
    cache_path: &Path,
) {
    if repos.contains_key(&record) {
        return;
    }

    match Repository::from_cache_or_url(&record.url, record.channel.clone(), cache_path) {
        Ok(repo) => {
            for url in repo.meta().linked_repositories.iter() {
                recurse_linked_repos(url, record.channel.clone(), repos, cache_path);
            }

            repos.insert(record.clone(), repo);
        }
        // TODO: actual error handling omg
        Err(e) => {
            eprintln!("{:?}", e);
        }
    };
}

pub(crate) fn find_package_by_id(
    repos: &HashMap<RepoRecord, Repository>,
    package_id: &str,
) -> Option<(AbsolutePackageKey, Package)> {
    repos.iter().find_map(|(key, repo)| {
        repo.packages().get(package_id).map(|x| {
            (
                AbsolutePackageKey::new(repo.meta(), &key.channel, package_id),
                x.to_owned(),
            )
        })
    })
}
