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
use crate::AbsolutePackageKey;

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Repository {
    meta: RepositoryMeta,
    packages: Packages,
    virtuals: Virtuals,

    channel: String,

    #[serde(skip_serializing)]
    hash_id: String
}

impl Repository {
    pub fn from_url(url: &Url, channel: String) -> Result<Repository, RepoDownloadError> {
        let client = reqwest::Client::new();

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

        let mut sha = Sha256::new();
        sha.input_str(&meta.base);
        let hash_id = sha.result_str();

        Ok(Repository {
            meta,
            packages,
            virtuals,
            channel,
            hash_id
        })
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

    pub fn hash_id(&self) -> &str {
        &self.hash_id
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
    JsonError(serde_json::Error)
}

impl std::fmt::Display for RepoDownloadError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match *self {
            RepoDownloadError::ReqwestError(ref e) => e.fmt(f),
            RepoDownloadError::JsonError(ref e) => e.fmt(f)
        }
    }
}
