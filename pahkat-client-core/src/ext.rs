use std::{
    convert::TryFrom,
    path::{Path, PathBuf},
};

use sha2::digest::Digest;
use sha2::Sha256;
use types::{package_key::TryFromError, repo::RepoUrl, DependencyKey, PackageKey};

pub(crate) trait PathExt {
    fn join_sha256(&self, bytes: &[u8]) -> PathBuf;
}

impl PathExt for Path {
    fn join_sha256(&self, bytes: &[u8]) -> PathBuf {
        let mut sha = Sha256::new();
        sha.update(bytes);
        let hash_id = format!("{:x}", sha.finalize());
        let part1 = &hash_id[0..2];
        let part2 = &hash_id[2..4];
        let part3 = &hash_id[4..];
        self.join(part1).join(part2).join(part3)
    }
}

pub(crate) trait DependencyKeyExt {
    fn into_package_key(self, repo_url: &RepoUrl) -> Result<PackageKey, TryFromError>;
    fn to_package_key(&self, repo_url: &RepoUrl) -> Result<PackageKey, TryFromError>;
}

impl DependencyKeyExt for DependencyKey {
    fn into_package_key(self, repo_url: &RepoUrl) -> Result<PackageKey, TryFromError> {
        match self {
            DependencyKey::Remote(url) => PackageKey::try_from(url),
            DependencyKey::Local(id) => Ok(PackageKey::new_unchecked(repo_url.clone(), id, None)),
        }
    }

    fn to_package_key(&self, repo_url: &RepoUrl) -> Result<PackageKey, TryFromError> {
        match self {
            DependencyKey::Remote(url) => PackageKey::try_from(url),
            DependencyKey::Local(id) => Ok(PackageKey::new_unchecked(
                repo_url.clone(),
                id.to_string(),
                None,
            )),
        }
    }
}
