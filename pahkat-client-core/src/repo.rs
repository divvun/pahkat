use hashbrown::HashMap;
use pahkat_types::Package;
use serde::{Deserialize, Serialize};
use sha2::digest::Digest;
use sha2::Sha256;
use std::path::Path;
use std::sync::{Arc, RwLock};
use url::Url;

mod package_key;
mod repository;

pub use package_key::PackageKey;
pub use repository::{RepoDownloadError, Repository};

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq, Hash)]
pub struct RepoRecord {
    pub url: Url,
    pub channel: String,
}

pub(crate) fn download(
    config: &Arc<RwLock<StoreConfig>>,
    package_key: &PackageKey,
    repos: &Arc<RwLock<HashMap<RepoRecord, Repository>>>,
    progress: Box<dyn Fn(u64, u64) -> bool + Send + 'static>,
) -> Result<std::path::PathBuf, crate::download::DownloadError> {
    use pahkat_types::Downloadable;

    let package = match find_package_by_key(package_key, repos) {
        Some(v) => v,
        None => {
            return Err(crate::download::DownloadError::NoUrl);
        }
    };

    let installer = match package.installer() {
        None => return Err(crate::download::DownloadError::NoUrl),
        Some(v) => v,
    };

    let url = match Url::parse(&*installer.url()) {
        Ok(v) => v,
        Err(e) => return Err(crate::download::DownloadError::InvalidUrl),
    };

    let config = config.read().unwrap();
    let dm = crate::download::DownloadManager::new(
        config.download_cache_path(),
        config.max_concurrent_downloads(),
    );

    let output_path = crate::repo::download_path(&*config, &installer.url());
    crate::block_on(dm.download(&url, output_path, Some(progress)))
}

pub(crate) fn download_path(config: &StoreConfig, url: &str) -> std::path::PathBuf {
    let mut sha = Sha256::new();
    sha.input(url.as_bytes());
    let hash_id = format!("{:x}", sha.result());
    let part1 = &hash_id[0..2];
    let part2 = &hash_id[2..4];
    let part3 = &hash_id[4..];

    config
        .package_cache_path()
        .join(part1)
        .join(part2)
        .join(part3)
}

use crate::transaction::{PackageStatus, PackageStatusError};
use std::collections::BTreeMap;

pub(crate) fn all_statuses<P, T>(
    store: &P,
    repo_record: &RepoRecord,
    target: &T,
) -> BTreeMap<String, Result<PackageStatus, PackageStatusError>>
where
    P: PackageStore<Target = T>,
    T: Send + Sync + std::fmt::Debug,
{
    log::debug!(
        "Getting all statuses for: {:?}, target: {:?}",
        repo_record,
        target
    );
    let mut map = BTreeMap::new();

    let repos = store.repos();
    let repos = repos.read().unwrap();

    if let Some(repo) = repos.get(repo_record) {
        for id in repo.packages().keys() {
            let key = PackageKey::new(repo.meta(), repo.channel(), id);
            let status = store.status(&key, target);
            log::trace!("Package: {:?}, status: {:?}", &id, &status);
            map.insert(id.clone(), status);
        }
    } else {
        log::warn!("Did not find repo {:?} in available repos", &repo_record);
        log::trace!("Repos available: {:?}", &*repos);
    }

    map
}

pub(crate) fn find_package_by_key(
    package_key: &PackageKey,
    repos: &Arc<RwLock<HashMap<RepoRecord, Repository>>>,
) -> Option<Package> {
    log::trace!("Resolving package...");
    log::trace!("My pkg id: {}", &package_key.id);
    repos
        .read()
        .unwrap()
        .get(&RepoRecord {
            url: package_key.url.clone(),
            channel: package_key.channel.clone(),
        })
        .and_then(|r| {
            log::trace!("Got repo: {:?}", r);
            let pkg = match r.packages().get(&package_key.id) {
                Some(x) => Some(x.to_owned()),
                None => None,
            };
            log::trace!("Found pkg: {:?}", &pkg);
            pkg
        })
}

pub(crate) fn find_package_by_id<P, T>(
    store: &P,
    package_id: &str,
    repos: &Arc<RwLock<HashMap<RepoRecord, Repository>>>,
) -> Option<(PackageKey, Package)>
where
    P: PackageStore<Target = T>,
    T: Send + Sync,
{
    match PackageKey::from_string(package_id) {
        Ok(k) => return store.find_package_by_key(&k).map(|pkg| (k, pkg)),
        Err(_) => {}
    };

    repos.read().unwrap().iter().find_map(|(key, repo)| {
        repo.packages().get(package_id).map(|x| {
            (
                PackageKey::new(repo.meta(), &key.channel, package_id),
                x.to_owned(),
            )
        })
    })
}

use crate::StoreConfig;

pub(crate) fn refresh_repos(config: &StoreConfig) -> HashMap<RepoRecord, Repository> {
    let mut repos = HashMap::new();

    log::debug!("Refreshing repos...");

    for record in config.repos().iter() {
        log::trace!("{:?}", &record);
        recurse_repo(record, &mut repos, &config.repo_cache_path());
    }

    repos
}

pub(crate) fn clear_cache(config: &StoreConfig) {
    for record in config.repos().iter() {
        match Repository::clear_cache(
            &record.url,
            record.channel.clone(),
            &config.repo_cache_path(),
        ) {
            Err(e) => {
                log::error!("{:?}", e);
            }
            Ok(_) => {}
        };
    }
}

use crate::package_store::PackageStore;
use crate::transaction::PackageDependencyError;

fn recurse_package_dependencies<T>(
    store: &Arc<dyn PackageStore<Target = T>>,
    package: &Package,
    candidates: &mut HashMap<PackageKey, Package>,
) -> Result<(), PackageDependencyError>
where
    T: Send + Sync,
{
    for (package_key, _version) in package.dependencies.iter() {
        // Package key may be a short, relative package id, or a fully qualified
        // URL to a package in a linked repo

        let result = store.find_package_by_id(package_key);

        match result {
            Some((key, package)) => {
                if candidates.contains_key(&key) {
                    continue;
                }

                recurse_package_dependencies(store, &package, candidates)?;
                candidates.insert(key, package);
            }
            None => {
                return Err(PackageDependencyError::PackageNotFound(
                    package_key.to_string(),
                ))
            }
        };
    }

    Ok(())
}

pub(crate) fn find_package_dependencies<T>(
    store: &Arc<dyn PackageStore<Target = T>>,
    _key: &PackageKey,
    package: &Package,
    _target: &T,
) -> Result<Vec<(PackageKey, Package)>, PackageDependencyError>
where
    T: Send + Sync,
{
    let mut candidates = HashMap::new();
    recurse_package_dependencies(store, &package, &mut candidates)?;
    Ok(candidates.into_iter().map(|(k, v)| (k, v)).collect())
    // for (package_id, version) in package.dependencies.iter() {
    //     // avoid circular references by keeping
    //     // track of package ids that have already been processed
    //     if resolved.contains(package_id) {
    //         continue;
    //     }
    //     resolved.push(package_id.clone());

    //     match self.find_package_by_id(package_id.as_str()) {
    //         Some((ref key, ref package)) => {
    //             // add all the dependencies of the dependency
    //             // to the list result first
    //             for dependency in
    //                 self.find_package_dependencies_impl(key, package, target, level + 1, resolved)?
    //             {
    //                 push_if_not_exists(dependency, &mut result);
    //             }

    //             // make sure the version requirement is correct
    //             // TODO: equality isn't how version comparisons work.
    //             // if package.version.as_str() != version {
    //             //     return Err(PackageDependencyError::VersionNotFound);
    //             // }

    //             match self.status(key, &target) {
    //                 Err(error) => return Err(PackageDependencyError::PackageStatusError(error)),
    //                 Ok(status) => match status {
    //                     PackageStatus::UpToDate => {}
    //                     _ => {
    //                         let dependency = PackageDependency {
    //                             id: key.clone(),
    //                             package: package.clone(),
    //                             version: version.clone(),
    //                             status,
    //                         };
    //                         push_if_not_exists(dependency, &mut result);
    //                     }
    //                 },
    //             }
    //         }
    //         _ => {
    //             // the given package id does not exist
    //             return Err(PackageDependencyError::PackageNotFound);
    //         }
    //     }
    // }

    // return Ok(result);
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
            log::error!("{:?}", e);
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
            log::error!("{:?}", e);
        }
    };
}
