use pahkat_types::{
    Package, PackageMap, Packages, Repository as RepositoryMeta, VirtualMap, Virtuals,
};
use serde::{Deserialize, Serialize};
use sha2::digest::Digest;
use sha2::Sha256;
use std::fs::{self, File};
use std::path::Path;
use std::time::SystemTime;
use url::Url;

fn last_modified_cache(repo_cache_path: &Path) -> SystemTime {
    match std::fs::metadata(repo_cache_path.join("cache.json")) {
        Ok(v) => match v.modified() {
            Ok(v) => v,
            Err(_) => std::time::SystemTime::UNIX_EPOCH,
        },
        Err(_) => std::time::SystemTime::UNIX_EPOCH,
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

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Repository {
    meta: RepositoryMeta,
    packages: Packages,
    virtuals: Virtuals,
    channel: String,
    hash_id: String,
}

impl Repository {
    pub fn path_hash(url: &Url, channel: &str) -> String {
        let mut sha = Sha256::new();
        sha.input(&format!("{}#{}", &url, &channel));
        format!("{:x}", sha.result())
    }

    pub fn from_cache_or_url(
        url: &Url,
        channel: String,
        cache_path: &Path,
    ) -> Result<Repository, RepoDownloadError> {
        log::debug!("{}, {}, {:?}", url, &channel, cache_path);
        let hash_id = Repository::path_hash(url, &channel);

        let repo_cache_path = cache_path.join(&hash_id);

        if !repo_cache_path.exists() {
            log::debug!("Cache does not exist, creating");
            let repo = Repository::from_url(url, channel)?;
            repo.save_to_cache(cache_path)
                .map_err(|e| RepoDownloadError::IoError(e))?;
            log::debug!("Save repo");
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
            log::debug!("Loading from cache");
            match Repository::from_directory(cache_path, hash_id) {
                Ok(v) => return Ok(v),
                Err(_) => {} // fallthrough
            }
        }

        log::debug!("loading from web");
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
        let client = reqwest::blocking::Client::new();
        let hash_id = Repository::path_hash(url, &channel);

        let meta_res = client
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

        let pkg_res = client
            .get(&format!("{}/packages/{}", url, index_json_path))
            .send()
            .map_err(|e| RepoDownloadError::ReqwestError(e))?;
        let pkg_text = pkg_res
            .text()
            .map_err(|e| RepoDownloadError::ReqwestError(e))?;
        let packages: Packages =
            serde_json::from_str(&pkg_text).map_err(|e| RepoDownloadError::JsonError(e))?;

        let virt_res = client
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

    pub fn channel(&self) -> &str {
        &self.channel
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
