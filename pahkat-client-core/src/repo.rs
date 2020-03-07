use hashbrown::HashMap;
use serde::{Deserialize, Serialize};
use sha2::digest::Digest;
use sha2::Sha256;
use std::path::Path;
use std::sync::{Arc, RwLock};
use url::Url;

mod package_key;
mod repository;

pub use package_key::PackageKey;
pub use repository::{LoadedRepository, RepoDownloadError};

use crate::config::Config;
use crate::package_store::PackageStore;
use crate::transaction::PackageDependencyError;

use pahkat_types::package::{self, Package};
use thiserror::Error;

use crate::defaults;
use crate::transaction::{PackageStatus, PackageStatusError};
use std::collections::BTreeMap;

#[derive(Debug, Clone, Error)]
pub enum PayloadError {
    #[error("No package found")]
    NoPackage,
    #[error("Invalid package found")]
    NoConcretePackage,
    #[error("No payload found meeting query criteria")]
    NoPayloadFound,
    #[error("Some criteria is not met for the current payload")]
    CriteriaUnmet(String),
}

use pahkat_types::package::Version;
use std::convert::TryInto;

#[derive(Debug, Clone)]
pub struct ReleaseQuery<'a> {
    pub platform: &'a str,
    pub arch: Option<&'a str>,
    pub channels: Vec<&'a str>,
    pub versions: Vec<VersionQuery<'a>>,
    pub payloads: Vec<&'a str>,
}

impl<'a> Default for ReleaseQuery<'a> {
    fn default() -> Self {
        Self {
            platform: defaults::platform(),
            arch: defaults::arch(),
            channels: vec![],
            versions: vec![],
            payloads: defaults::payloads().to_vec(),
        }
    }
}

#[derive(Debug, Clone)]
#[non_exhaustive]
pub enum VersionQuery<'a> {
    Match(&'a str),
    Semantic(&'a str),
    Timestamp(&'a str),
}

impl<'a> VersionQuery<'a> {
    const fn any_semantic() -> Self {
        VersionQuery::Semantic("*")
    }

    fn matches(&self, version: &Version) -> bool {
        match (self, version) {
            (VersionQuery::Semantic(mask), Version::Semantic(v)) => {
                if *mask == "*" {
                    true
                } else {
                    todo!()
                }
            }
            _ => todo!(),
        }
    }
}

#[inline(always)]
fn empty_payloads() -> &'static [&'static str] {
    &[]
}

impl<'a> From<&'a PackageKey> for ReleaseQuery<'a> {
    fn from(key: &'a PackageKey) -> Self {
        ReleaseQuery {
            platform: key
                .platform
                .as_ref()
                .map(|x| &**x)
                .unwrap_or_else(|| defaults::platform()),
            arch: key.arch.as_ref().map(|x| &**x).or_else(|| defaults::arch()),
            channels: vec![&*key.channel],
            versions: key
                .version
                .as_ref()
                .map(|v| vec![VersionQuery::Match(&*v)])
                .unwrap_or_else(|| vec![]),
            payloads: defaults::payloads().to_vec(),
        }
    }
}

pub(crate) struct ReleaseQueryIter<'a> {
    query: &'a ReleaseQuery<'a>,
    descriptor: &'a pahkat_types::package::Descriptor,
    next_release: usize,
}

use pahkat_types::{
    package::Release,
    payload::{Payload, Target},
};

#[derive(Debug, Clone)]
pub(crate) struct ReleaseQueryResponse<'a> {
    pub release: &'a Release,
    pub target: &'a Target,
}

impl<'a> ReleaseQueryIter<'a> {
    #[inline(always)]
    fn next_release(&mut self) -> Option<ReleaseQueryResponse<'a>> {
        while let Some(release) = self.descriptor.releases.get(self.next_release) {
            if !self.query.channels.is_empty() && !self.query.channels.contains(&&*release.channel)
            {
                self.next_release += 1;
                continue;
            }

            if !self.query.versions.is_empty()
                && self
                    .query
                    .versions
                    .iter()
                    .find(|q| q.matches(&release.version))
                    .is_none()
            {
                self.next_release += 1;
                continue;
            }

            if let Some(payload) = self.next_payload(release) {
                self.next_release += 1;
                return Some(payload);
            }

            self.next_release += 1;
            continue;
        }

        None
    }

    #[inline(always)]
    fn next_payload(&mut self, release: &'a Release) -> Option<ReleaseQueryResponse<'a>> {
        for ref target in release.targets.iter() {
            if target.platform != self.query.platform {
                continue;
            }

            if let Some(arch) = self.query.arch {
                if let Some(ref target_arch) = target.arch {
                    if target_arch != arch {
                        continue;
                    }
                }
            } else if target.arch.is_some() {
                continue;
            }

            return Some(ReleaseQueryResponse { release, target });
        }

        None
    }
}

impl<'a> Iterator for ReleaseQueryIter<'a> {
    type Item = ReleaseQueryResponse<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        self.next_release()
    }
}

impl<'a> ReleaseQuery<'a> {
    fn semver(channels: &'a [&'a str]) -> ReleaseQuery<'a> {
        const ANY_SEMANTIC: &[VersionQuery<'static>] = &[VersionQuery::any_semantic()];

        ReleaseQuery {
            versions: ANY_SEMANTIC.to_vec(),
            ..Default::default()
        }
    }

    pub(crate) fn iter(
        &'a self,
        descriptor: &'a pahkat_types::package::Descriptor,
    ) -> ReleaseQueryIter<'a> {
        ReleaseQueryIter {
            query: self,
            descriptor,
            next_release: 0,
        }
    }
}

pub(crate) fn resolve_payload<'a>(
    package_key: &PackageKey,
    query: ReleaseQuery<'a>,
    repos: &'a HashMap<Url, LoadedRepository>,
) -> Result<
    (
        pahkat_types::payload::Target,
        pahkat_types::package::Release,
        pahkat_types::package::Descriptor,
    ),
    PayloadError,
> {
    let package = find_package_by_key(package_key, repos).ok_or(PayloadError::NoPackage)?;
    let descriptor: &pahkat_types::package::Descriptor = &package
        .try_into()
        .map_err(|_| PayloadError::NoConcretePackage)?;
    query
        .iter(&descriptor)
        .next()
        .map(|x| (x.target.clone(), x.release.clone(), descriptor.clone()))
        .ok_or(PayloadError::NoPayloadFound)
}

pub(crate) fn download<'a>(
    config: &Arc<RwLock<Config>>,
    package_key: &PackageKey,
    query: ReleaseQuery<'a>,
    repos: &HashMap<Url, LoadedRepository>,
    progress: Box<dyn Fn(u64, u64) -> bool + Send + 'static>,
) -> Result<std::path::PathBuf, crate::download::DownloadError> {
    use pahkat_types::AsDownloadUrl;

    let (target, _, _) = match resolve_payload(package_key, query, repos) {
        Ok(v) => v,
        Err(e) => return Err(crate::download::DownloadError::Payload(e)),
    };

    let url = target.payload.as_download_url();

    let config = config.read().unwrap();
    let settings = config.settings();
    let dm = crate::download::DownloadManager::new(
        settings.download_cache_dir().to_path_buf(),
        settings.max_concurrent_downloads(),
    );

    let output_path = crate::repo::download_dir(&*config, url);
    crate::block_on(dm.download(url, output_path, Some(progress)))
}

pub(crate) fn download_dir(config: &Config, url: &url::Url) -> std::path::PathBuf {
    let mut sha = Sha256::new();
    sha.input(url.as_str().as_bytes());
    let hash_id = format!("{:x}", sha.result());
    let part1 = &hash_id[0..2];
    let part2 = &hash_id[2..4];
    let part3 = &hash_id[4..];

    config
        .settings()
        .package_cache_dir()
        .join(part1)
        .join(part2)
        .join(part3)
}

pub(crate) fn download_file_path(config: &Config, url: &url::Url) -> std::path::PathBuf {
    download_dir(config, url).join(
        url.path_segments()
            .unwrap_or_else(|| "".split('/'))
            .last()
            .unwrap(),
    )
}

pub(crate) fn all_statuses<'a, P, T>(
    store: &P,
    repo_url: &Url,
    target: &T,
) -> BTreeMap<String, Result<PackageStatus, PackageStatusError>>
where
    P: PackageStore<Target = T>,
    T: Send + Sync + std::fmt::Debug,
{
    log::debug!(
        "Getting all statuses for: {:?}, target: {:?}",
        repo_url,
        target
    );
    let mut map = BTreeMap::new();

    let repos = store.repos();
    let repos = repos.read().unwrap();

    if let Some(repo) = repos.get(repo_url) {
        for id in repo.packages().packages.keys() {
            let key = PackageKey::unchecked_new(
                repo.info().base_url.clone(),
                id.clone(),
                repo.meta().channel.clone(),
                None,
            );
            let status = store.status(&key, target);
            log::trace!("Package: {:?}, status: {:?}", &id, &status);
            map.insert(id.clone(), status);
        }
    } else {
        log::warn!("Did not find repo {:?} in available repos", &repo_url);
        log::trace!("Repos available: {:?}", &*repos);
    }

    map
}

pub(crate) fn find_package_by_key<'p>(
    package_key: &PackageKey,
    repos: &'p HashMap<Url, LoadedRepository>,
) -> Option<Package> {
    log::trace!("Resolving package...");
    log::trace!("My pkg id: {}", &package_key.id);
    repos.get(&package_key.repository_url).and_then(|r| {
        log::trace!("Got repo: {:?}", r);
        // TODO: need to check that any release supports the requested channel
        let pkg = match r.packages().packages.get(&package_key.id) {
            Some(x) => Some(x.to_owned()),
            None => None,
        };
        log::trace!("Found pkg: {:?}", &pkg);
        pkg
    })
}

use std::convert::TryFrom;

pub(crate) fn find_package_by_id<P, T>(
    store: &P,
    package_id: &str,
    repos: &HashMap<Url, LoadedRepository>,
) -> Option<(PackageKey, Package)>
where
    P: PackageStore<Target = T>,
    T: Send + Sync,
{
    match PackageKey::try_from(package_id) {
        Ok(k) => return store.find_package_by_key(&k).map(|pkg| (k, pkg)),
        Err(_) => {}
    };

    repos.iter().find_map(|(key, repo)| {
        repo.packages().packages.get(package_id).map(|x| {
            let key = PackageKey::unchecked_new(
                repo.info().base_url.clone(),
                package_id.to_string(),
                repo.meta().channel.clone(),
                None,
            );
            (key, x.to_owned())
        })
    })
}

pub(crate) fn refresh_repos(config: &Arc<RwLock<Config>>) -> HashMap<Url, LoadedRepository> {
    let mut repos = HashMap::new();

    log::debug!("Refreshing repos...");

    let config = config.read().unwrap();

    for url in config.repos().keys() {
        log::trace!("{:?}", &url);
        recurse_repo(&url, &mut repos, &config.settings().repo_cache_dir());
    }

    repos
}

pub(crate) fn clear_cache(config: &Arc<RwLock<Config>>) {
    // for record in config.repos().iter() {
    //     match LoadedRepository::clear_cache(
    //         &record.url,
    //         record.channel.clone(),
    //         &config.repo_cache_dir(),
    //     ) {
    //         Err(e) => {
    //             log::error!("{:?}", e);
    //         }
    //         Ok(_) => {}
    //     };
    // }
    todo!()
}

fn recurse_package_dependencies<T>(
    store: &Arc<dyn PackageStore<Target = T>>,
    package: &Package,
    candidates: &mut HashMap<PackageKey, Package>,
) -> Result<(), PackageDependencyError>
where
    T: Send + Sync,
{
    // for (package_key, _version) in package.dependencies.iter() {
    //     // Package key may be a short, relative package id, or a fully qualified
    //     // URL to a package in a linked repo

    //     let result = store.find_package_by_id(package_key);

    //     match result {
    //         Some((key, package)) => {
    //             if candidates.contains_key(&key) {
    //                 continue;
    //             }

    //             recurse_package_dependencies(store, &package, candidates)?;
    //             candidates.insert(key, package);
    //         }
    //         None => {
    //             return Err(PackageDependencyError::PackageNotFound(
    //                 package_key.to_string(),
    //             ))
    //         }
    //     };
    // }

    // Ok(())
    todo!()
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

fn recurse_repo(url: &Url, repos: &mut HashMap<Url, LoadedRepository>, cache_dir: &Path) {
    if repos.contains_key(&url) {
        return;
    }

    match LoadedRepository::from_cache_or_url(url, cache_dir) {
        Ok(repo) => {
            for url in repo.info().linked_repositories.iter() {
                recurse_repo(url, repos, cache_dir);
            }

            repos.insert(url.clone(), repo);
        }
        // TODO: actual error handling omg
        Err(e) => {
            log::error!("{:?}", e);
        }
    };
}
