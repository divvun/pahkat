use crypto::digest::Digest;
use crypto::sha2::Sha256;
use serde_derive::Serialize;
use pahkat::types::{
    Package,
    Packages,
    Virtuals,
    PackageMap,
    VirtualRefMap,
    Repository as RepositoryMeta
};
use url::Url;
use std::path::Path;
use crate::AbsolutePackageKey;
use std::time::SystemTime;
use std::fs::{self, File};

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Repository {
    meta: RepositoryMeta,
    packages: Packages,
    virtuals: Virtuals,
    channel: String,
    hash_id: String
}

fn last_modified_cache(repo_cache_path: &Path) -> SystemTime {
    match std::fs::metadata(repo_cache_path.join("cache.json")) {
        Ok(v) => {
            match v.modified() {
                Ok(v) => v,
                Err(_) => std::time::SystemTime::UNIX_EPOCH
            }
        },
        Err(_) => {
            std::time::SystemTime::UNIX_EPOCH
        }
    }
}

impl Repository {
    pub fn path_hash(url: &Url, channel: &str) -> String {
        let mut sha = Sha256::new();
        sha.input_str(&format!("{}#{}", &url, &channel));
        sha.result_str()
    }

    pub fn from_cache_or_url(url: &Url, channel: String, cache_path: &Path) -> Result<Repository, RepoDownloadError> {
        let hash_id = Repository::path_hash(url, &channel);

        let repo_cache_path = cache_path.join(&hash_id);

        if !repo_cache_path.exists() {
            let repo = Repository::from_url(url, channel)?;
            repo.save_to_cache(cache_path)
                .map_err(|e| RepoDownloadError::IoError(e))?;
            return Ok(repo);
        }

        // Check cache age
        let ts = last_modified_cache(&repo_cache_path);
        
        // 5 minutes cache check
        let is_cache_valid = match SystemTime::now().duration_since(ts) {
            Ok(v) => v.as_secs() < 300,
            Err(_) => false
        };

        if is_cache_valid {
            Repository::from_directory(&repo_cache_path, hash_id) 
        } else {
            let repo = Repository::from_url(url, channel)?;
            repo.save_to_cache(cache_path)
                .map_err(|e| RepoDownloadError::IoError(e))?;
            Ok(repo)
        }
    }

    fn from_directory(cache_path: &Path, hash_id: String) -> Result<Repository, RepoDownloadError> {
        let cache_file = File::open(cache_path.join(&hash_id).join("cache.json"))
            .map_err(|e| RepoDownloadError::IoError(e))?;
        
        let repo: Repository = serde_json::from_reader(cache_file)
            .map_err(|e| RepoDownloadError::JsonError(e))?;

        Ok(repo)

        // let meta_file = File::open(cache_path.join("index.json"))
        //     .map_err(|e| RepoDownloadError::IoError(e))?;
        // let packages_file = File::open(cache_path.join("packages/index.json"))
        //     .map_err(|e| RepoDownloadError::IoError(e))?;
        // let virtuals_file = File::open(cache_path.join("virtuals/index.json"))
        //     .map_err(|e| RepoDownloadError::IoError(e))?;

        // let meta = serde_json::from_reader(meta_file)
        //     .map_err(|e| RepoDownloadError::JsonError(e))?;;
        // let packages = serde_json::from_reader(packages_file)
        //     .map_err(|e| RepoDownloadError::JsonError(e))?;;
        // let virtuals = serde_json::from_reader(virtuals_file)
        //     .map_err(|e| RepoDownloadError::JsonError(e))?;;

        // Ok(Repository {
        //     meta,
        //     packages,
        //     virtuals,
        //     channel,
        //     hash_id
        // })
    }

    pub fn from_url(url: &Url, channel: String) -> Result<Repository, RepoDownloadError> {
        let client = reqwest::Client::new();

        let mut sha = Sha256::new();
        sha.input_str(&format!("{}#{}", &url, &channel));
        let hash_id = sha.result_str();

        let mut meta_res = client.get(&format!("{}/index.json", url)).send()
            .map_err(|e| RepoDownloadError::ReqwestError(e))?;
        let meta_text = meta_res.text()
            .map_err(|e| RepoDownloadError::ReqwestError(e))?;
        let meta: RepositoryMeta = serde_json::from_str(&meta_text)
            .map_err(|e| RepoDownloadError::JsonError(e))?;

        let mut pkg_res = client.get(&format!("{}/packages/index.json", url)).send()
            .map_err(|e| RepoDownloadError::ReqwestError(e))?;
        let pkg_text = pkg_res.text()
            .map_err(|e| RepoDownloadError::ReqwestError(e))?;
        let packages: Packages = serde_json::from_str(&pkg_text)
            .map_err(|e| RepoDownloadError::JsonError(e))?;

        let mut virt_res = client.get(&format!("{}/virtuals/index.json", url)).send()
            .map_err(|e| RepoDownloadError::ReqwestError(e))?;
        let virt_text = virt_res.text()
            .map_err(|e| RepoDownloadError::ReqwestError(e))?;
        let virtuals: Virtuals = serde_json::from_str(&virt_text)
            .map_err(|e| RepoDownloadError::JsonError(e))?;

        let repo = Repository {
            meta,
            packages,
            virtuals,
            channel,
            hash_id
        };

        Ok(repo)
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

    pub fn virtuals(&self) -> &VirtualRefMap {
        &self.virtuals.virtuals
    }
}


#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct PackageRecord {
    id: AbsolutePackageKey,
    package: Package
}

impl PackageRecord {
    pub fn new(repo: &RepositoryMeta, channel: &str, package: Package) -> PackageRecord {
        PackageRecord {
            id: AbsolutePackageKey {
                url: Url::parse(&repo.base).expect("repo base url must be valid"),
                id: package.id.to_string(),
                channel: channel.to_string()
            },
            package
        }
    }

    pub fn id(&self) -> &AbsolutePackageKey {
        &self.id
    }

    pub fn package(&self) -> &Package {
        &self.package
    }
}

#[derive(Debug)]
pub enum RepoDownloadError {
    ReqwestError(reqwest::Error),
    JsonError(serde_json::Error),
    IoError(std::io::Error)
}

impl std::fmt::Display for RepoDownloadError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match *self {
            RepoDownloadError::ReqwestError(ref e) => e.fmt(f),
            RepoDownloadError::JsonError(ref e) => e.fmt(f),
            RepoDownloadError::IoError(ref e) => e.fmt(f)
        }
    }
}
