mod package_key;
mod repository;

pub use package_key::PackageKey;
pub use repository::{LoadedRepository, RepoDownloadError};

use std::collections::BTreeMap;
use std::convert::{TryFrom, TryInto};
use std::path::Path;
use std::pin::Pin;
use std::sync::{Arc, RwLock};

use dashmap::DashMap;
use futures::future::{Future, FutureExt};
use hashbrown::HashMap;
use sha2::digest::Digest;
use sha2::Sha256;
use thiserror::Error;
use url::Url;

use crate::config::Config;
use crate::defaults;
use crate::fbs::PackagesExt;
use crate::package_store::PackageStore;
use crate::transaction::{PackageDependencyError, PackageStatus, PackageStatusError};
use pahkat_types::package::{Package, Release, Version, Descriptor};
use pahkat_types::payload::Target;

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

#[derive(Debug, Clone)]
pub struct ReleaseQuery<'a> {
    pub platform: &'a str,
    pub arch: Option<&'a str>,
    pub channels: Vec<&'a str>,
    pub versions: Vec<VersionQuery<'a>>,
    pub payloads: Vec<&'a str>,
}

impl<'a> ReleaseQuery<'a> {
    pub(crate) fn and_payloads(mut self, payloads: Vec<&'a str>) -> Self {
        self.payloads = payloads;
        self
    }
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

pub(crate) struct ReleaseQueryIter<'a> {
    query: &'a ReleaseQuery<'a>,
    descriptor: &'a pahkat_types::package::Descriptor,
    next_release: usize,
}

#[derive(Debug, Clone)]
pub(crate) struct ReleaseQueryResponse<'a> {
    pub release: &'a Release,
    pub target: &'a Target,
}

impl<'a> ReleaseQueryIter<'a> {
    #[inline(always)]
    fn next_release(&mut self) -> Option<ReleaseQueryResponse<'a>> {
        log::trace!("Beginning release query iter: {:#?}", &self.query);

        while let Some(release) = self.descriptor.release.get(self.next_release) {
            log::trace!(
                "Candidate release: version:{:?}, channel:{:?}",
                &release.version.to_string(),
                &release.channel
            );

            // eprintln!("release: {:?}", &release);
            // If query is empty, it means search only for the main empty channel
            if let Some(channel) = release.channel.as_ref().map(|x| x.as_str()) {
                if !self.query.channels.contains(&channel) {
                    log::trace!("Skipping (not accepted channel)");
                    self.next_release += 1;
                    continue;
                }
            } else if release.channel.is_some() && !self.query.channels.is_empty() {
                log::trace!("Skipping (query channels not empty and no match)");
                self.next_release += 1;
                continue;
            }

            if let Some(payload) = self.next_payload(release) {
                log::trace!("Target resolved: {:#?}", &payload.target);
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
        for ref target in release.target.iter() {
            log::trace!(
                "Candidate target: platform:{} arch:{:?}",
                &target.platform,
                &target.arch
            );

            if target.platform != self.query.platform {
                log::trace!("Skipping (platform does not match)");
                continue;
            }

            if let Some(arch) = self.query.arch {
                if let Some(ref target_arch) = target.arch {
                    if target_arch != arch {
                        log::trace!("Skipping (arch does not match)");
                        continue;
                    }
                }
            } else if target.arch.is_some() {
                log::trace!("Skipping (no arch in query but arch in target)");
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

    pub fn new(key: &'a PackageKey, repos: &'a HashMap<Url, LoadedRepository>) -> Self {
        let channels = key
            .query
            .channel
            .as_ref()
            .map(|x| vec![&**x])
            .unwrap_or_else(|| {
                repos
                    .iter()
                    .find_map(|(url, repo)| {
                        log::trace!("ReleaseQuery::new() {} {}", &key.repository_url, url);
                        if &key.repository_url == url {
                            log::trace!("{:?}", repo.meta());
                            let channel = repo.meta().channel.as_ref().map(|x| &**x);
                            log::trace!("Channel? {:?}", &channel);
                            channel
                        } else {
                            None
                        }
                    })
                    .map(|x| vec![x])
                    .unwrap_or_else(|| vec![])
            });

        ReleaseQuery {
            platform: key
                .query
                .platform
                .as_ref()
                .map(|x| &**x)
                .unwrap_or_else(|| defaults::platform()),
            arch: key
                .query
                .arch
                .as_ref()
                .map(|x| &**x)
                .or_else(|| defaults::arch()),
            channels,
            versions: key
                .query
                .version
                .as_ref()
                .map(|v| vec![VersionQuery::Match(&*v)])
                .unwrap_or_else(|| vec![]),
            payloads: defaults::payloads().to_vec(),
        }
    }
}

pub(crate) fn resolve_package<'a>(
    package_key: &PackageKey,
    repos: &'a HashMap<Url, LoadedRepository>,
) -> Result<pahkat_types::package::Descriptor, PayloadError> {
    log::trace!("Finding package");
    let package = find_package_by_key(package_key, repos).ok_or(PayloadError::NoPackage)?;
    log::trace!("Package found");
    let descriptor: pahkat_types::package::Descriptor = package
        .try_into()
        .map_err(|_| PayloadError::NoConcretePackage)?;
    Ok(descriptor)
}

pub(crate) fn resolve_payload<'a>(
    package_key: &PackageKey,
    query: &ReleaseQuery<'a>,
    repos: &'a HashMap<Url, LoadedRepository>,
) -> Result<
    (
        pahkat_types::payload::Target,
        pahkat_types::package::Release,
        pahkat_types::package::Descriptor,
    ),
    PayloadError,
> {
    log::trace!("Resolving payload");
    let descriptor = resolve_package(package_key, repos)?;
    log::trace!("Package found");
    query
        .iter(&descriptor)
        .next()
        .map(|x| (x.target.clone(), x.release.clone(), descriptor.clone()))
        .ok_or(PayloadError::NoPayloadFound)
}

pub(crate) fn import<'a>(
    config: &Arc<RwLock<Config>>,
    package_key: &PackageKey,
    query: &ReleaseQuery<'a>,
    repos: &HashMap<Url, LoadedRepository>,
    installer_path: &Path,
) -> Result<std::path::PathBuf, crate::package_store::ImportError> {
    use pahkat_types::payload::AsDownloadUrl;
    log::debug!("IMPORTING");

    let (target, _, _) = resolve_payload(package_key, &query, &*repos)?;
    let config = config.read().unwrap();

    let output_path = download_file_path(&config, target.payload.as_download_url());
    log::debug!("DIR: {:?}", &installer_path);
    log::debug!("DIR: {:?}", &output_path);
    std::fs::create_dir_all(&output_path.parent().unwrap())?;
    std::fs::copy(installer_path, &output_path)?;
    Ok(output_path)
}

use crate::package_store::DownloadEvent;
use futures::stream::StreamExt;

// pub(crate) fn download<'a>(
//     config: &Arc<RwLock<Config>>,
//     package_key: &PackageKey,
//     query: &ReleaseQuery<'a>,
//     repos: &HashMap<Url, LoadedRepository>,
//     progress: Box<dyn Fn(u64, u64) -> bool + Send + 'static>,
// ) -> Result<std::path::PathBuf, crate::download::DownloadError> {
//     log::trace!("Downloading {} {:?}", package_key, &query);
//     use pahkat_types::AsDownloadUrl;

//     let (target, _, _) = match resolve_payload(package_key, &query, repos) {
//         Ok(v) => v,
//         Err(e) => {
//             return {
//                 log::error!(
//                     "Failed to resolve: {} {:?} {:?}",
//                     &package_key,
//                     &query,
//                     &repos
//                 );
//                 Err(crate::download::DownloadError::Payload(e))
//             }
//         }
//     };

//     let url = target.payload.as_download_url();

//     let config = config.read().unwrap();
//     let settings = config.settings();
//     let dm = crate::download::DownloadManager::new(
//         settings.download_cache_dir().to_path_buf(),
//         settings.max_concurrent_downloads(),
//     );

//     let output_path = crate::repo::download_dir(&*config, url);
//     crate::block_on(async move {
//         let mut dl = dm.download(url, output_path).await?;

//         while let Some(event) = dl.next().await {
//             match event {
//                 DownloadEvent::Error(e) => return Err(e),
//                 DownloadEvent::Complete(p) => return Ok(p),
//                 DownloadEvent::Progress((cur, total)) => {
//                     if !(progress)(cur, total) {
//                         break;
//                     }
//                 }
//             }
//         }

//         Err(crate::download::DownloadError::UserCancelled)
//     })
// }

#[must_use]
pub(crate) fn download<'a>(
    config: &Arc<RwLock<Config>>,
    package_key: &PackageKey,
    query: &ReleaseQuery<'a>,
    repos: &HashMap<Url, LoadedRepository>,
) -> std::pin::Pin<
    Box<
        dyn futures::stream::Stream<Item = crate::package_store::DownloadEvent>
            + Send
            + Sync
            + 'static,
    >,
> {
    log::trace!("Downloading {} {:?}", package_key, &query);
    use pahkat_types::AsDownloadUrl;

    let (target, _, _) = match resolve_payload(package_key, &query, repos) {
        Ok(v) => v,
        Err(e) => {
            log::error!(
                "Failed to resolve: {} {:?} {:?}",
                &package_key,
                &query,
                &repos
            );
            return Box::pin(async_stream::stream! {
                yield crate::package_store::DownloadEvent::Error(crate::download::DownloadError::Payload(e));
            });
        }
    };

    let url = target.payload.as_download_url().to_owned();

    let config = config.read().unwrap();
    let settings = config.settings();
    let dm = crate::download::DownloadManager::new(
        settings.download_cache_dir().to_path_buf(),
        settings.max_concurrent_downloads(),
    );

    let output_path = crate::repo::download_dir(&*config, &url);
    let stream = async_stream::stream! {
        match dm.download(&url, output_path).await {
            Ok(mut v) => {
                while let Some(value) = v.next().await {
                    yield value;
                }
            }
            Err(e) => {
                yield DownloadEvent::Error(e);
            }
        }
    };
    Box::pin(stream)
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

pub(crate) fn all_statuses<'a>(
    store: &dyn PackageStore,
    repo_url: &Url,
    target: crate::package_store::InstallTarget,
) -> BTreeMap<String, Result<PackageStatus, PackageStatusError>> {
    log::debug!(
        "Getting all statuses for: {:?}, target: {:?}",
        repo_url,
        target
    );
    let mut map = BTreeMap::new();

    let repos = store.repos();
    let repos = repos.read().unwrap();

    if let Some(repo) = repos.get(repo_url) {
        let packages = repo.packages();
        let packages = match packages.packages() {
            Some(v) => v,
            None => {
                log::error!("No packages map in fbs for {:?}!", &repo_url);
                return map;
            }
        };

        for id in packages.keys() {
            let key =
                PackageKey::unchecked_new(repo.info().repository.url.clone(), id.to_string(), None);
            let status = store.status(&key, target);
            log::trace!("Package: {:?}, status: {:?}", &id, &status);
            map.insert(id.to_string(), status);
        }
    } else {
        log::warn!("Did not find repo {:?} in available repos", &repo_url);
        log::trace!("Repos available: {:?}", &*repos);
    }

    map
}

pub(crate) async fn strings<'p>(
    repo_urls: Vec<Url>,
    language: String
) -> HashMap<Url, crate::package_store::LocalizedStrings> {
    let futures = repo_urls
        .into_iter()
        .map(|url| {
            let strings_url = url
                .join("strings/").unwrap()
                .join(&format!("{}.toml", language))
                .unwrap();
            (url, strings_url)
        })
        .map(|(url, strings_url)| async move {
            let (tx, rx) = tokio::sync::oneshot::channel();
            tokio::spawn(async move {
                let response = match reqwest::get(strings_url).await {
                    Ok(v) => match v.text().await {
                        Ok(v) => match toml::from_str(&v) {
                            Ok(v) => Some(v),
                            Err(_) => None,
                        },
                        Err(_) => None,
                    },
                    Err(_) => None,
                };
                tx.send(response).unwrap();
            });
            let result = rx.await.unwrap();

            (url, result)
        })
        .collect::<Vec<_>>();
    let results = futures::future::join_all(futures).await;

    results.into_iter().filter_map(|(k, v)| v.map(|v| (k, v))).collect::<HashMap<_, _>>()
}

pub(crate) fn find_package_by_key<'p>(
    package_key: &PackageKey,
    repos: &'p HashMap<Url, LoadedRepository>,
) -> Option<Package> {
    log::trace!("Resolving package...");
    log::trace!("My pkg id: {}", &package_key.id);
    log::trace!("Repo url: {}", &package_key.repository_url);
    log::trace!(
        "Repos: {:?}",
        repos.iter().map(|(x, _)| x).collect::<Vec<_>>()
    );

    repos.get(&package_key.repository_url).and_then(|r| {
        log::trace!("Got repo");
        // TODO: need to check that any release supports the requested channel
        let packages = r.packages();
        let packages = match packages.packages() {
            Some(v) => v,
            None => {
                log::error!(
                    "No packages map in fbs for {:?}!",
                    &package_key.repository_url
                );
                return None;
            }
        };

        let pkg = match packages.get(&package_key.id) {
            Some(x) => x,
            None => return None,
        };
        log::trace!("Found pkg");

        (&pkg).try_into().map(Package::Concrete).ok()
    })
}

pub(crate) fn find_package_by_id(
    store: &dyn PackageStore,
    package_id: &str,
    repos: &HashMap<Url, LoadedRepository>,
) -> Option<(PackageKey, Package)> {
    match PackageKey::try_from(package_id) {
        Ok(k) => return store.find_package_by_key(&k).map(|pkg| (k, pkg)),
        Err(_) => {}
    };

    repos.iter().find_map(|(key, repo)| {
        let packages = repo.packages();
        let packages = match packages.packages() {
            Some(v) => v,
            None => {
                log::error!("No packages map in fbs for {:?}!", &key);
                return None;
            }
        };

        packages.get(package_id).map(|x| {
            let key = PackageKey::unchecked_new(
                repo.info().repository.url.clone(),
                package_id.to_string(),
                None,
            );

            (&x).try_into().map(|p| (key, Package::Concrete(p))).ok()
        })?
    })
}

#[must_use]
pub(crate) async fn refresh_repos(
    config: Config,
) -> Result<HashMap<Url, LoadedRepository>, RepoDownloadError> {
    let config = Arc::new(config);

    log::debug!("Refreshing repos...");

    let repo_data = {
        let repo_keys =
            config
                .repos()
                .keys()
                .fold(crossbeam_queue::SegQueue::new(), |acc, cur| {
                    acc.push(cur.clone());
                    acc
                });

        workqueue::work(config, repo_keys, |url, queue, config| {
            Box::pin(async move {
                log::trace!("Downloading repo at {:?}…", &url);

                let cache_dir = config.settings().repo_cache_dir();
                let channel = config.repos().get(&url).and_then(|r| r.channel.clone());

                match LoadedRepository::from_cache_or_url(url, channel, cache_dir).await {
                    Ok(repo) => {
                        for url in repo.info().repository.linked_repositories.iter() {
                            log::trace!("Queuing linked repo: {:?}", &url);
                            queue.push(url.clone());
                            // recurse_repo(url.clone(), Arc::clone(&repos), Arc::clone(&config)).await?;
                        }

                        Ok(repo)
                    }
                    Err(e) => {
                        log::error!("{:?}", e);
                        Err(e)
                    }
                }
            })
        })
        .await
        .unwrap()
    };

    let mut map = HashMap::new();

    for (key, value) in repo_data.into_iter() {
        log::debug!("Resolved repository: {:?}", &key);

        match value {
            Ok(v) => {
                map.insert(key, v);
            }
            Err(e) => return Err(e),
        }
    }

    Ok(map)
}

pub(crate) fn clear_cache(config: &Arc<RwLock<Config>>) {
    // todo!()
}

#[derive(Debug, Clone)]
pub struct PackageCandidate {
    pub package_key: PackageKey,
    pub descriptor: Descriptor,
    pub release: Release,
    pub target: Target,
    pub status: PackageStatus,
    pub is_reboot_required: bool,
}

#[derive(Debug, thiserror::Error)]
pub enum PackageCandidateError {
    #[error("Could not resolve package status for package key: `{0}`")]
    Status(PackageKey, #[source] PackageStatusError),

    #[error("Could not resolve payload for package key: `{0}`")]
    Payload(PackageKey, #[source] PayloadError),

    #[error("Could not resolve identifier to package key: `{0}`")]
    UnresolvedId(String),

    #[error("Attempting to uninstall package required by installation set: `{0}`")]
    UninstallConflict(PackageKey),
}

use crate::package_store::InstallTarget;

fn resolve_package_candidate(
    store: &dyn PackageStore,
    package_key: &PackageKey,
    install_target: &[InstallTarget],
    repos: &HashMap<Url, LoadedRepository>,
) -> Result<PackageCandidate, PackageCandidateError> {
    let query = crate::repo::ReleaseQuery::new(package_key, &repos);

    let status = install_target.iter().fold(None, |acc, cur| {
        match acc {
            Some(Ok(v)) => Some(Ok(v)),
            _ => {
                Some(store.status(&package_key, *cur)
                    .map_err(|e| PackageCandidateError::Status(package_key.to_owned(), e)))
            }
        }
    }).unwrap_or_else(|| Err(PackageCandidateError::UnresolvedId(package_key.to_string())))?;

    let (target, release, descriptor) = resolve_payload(package_key, &query, &*repos)
        .map_err(|e| PackageCandidateError::Payload(package_key.to_owned(), e))?;

    use crate::transaction::PackageStatus;
    use pahkat_types::payload::Payload;

    let is_reboot_required = match &target.payload {
        Payload::TarballPackage(_) => false,
        Payload::MacOSPackage(pkg) => {
            use pahkat_types::payload::macos::RebootSpec;
            match status {
                PackageStatus::NotInstalled => pkg.requires_reboot.contains(&RebootSpec::Install),
                PackageStatus::RequiresUpdate => pkg.requires_reboot.contains(&RebootSpec::Update),
                _ => false,
            }
        }
        Payload::WindowsExecutable(pkg) => {
            use pahkat_types::payload::windows::RebootSpec;
            match status {
                PackageStatus::NotInstalled => pkg.requires_reboot.contains(&RebootSpec::Install),
                PackageStatus::RequiresUpdate => pkg.requires_reboot.contains(&RebootSpec::Update),
                _ => false,
            }
        },
        _ => false
    };

    Ok(PackageCandidate {
        package_key: package_key.to_owned(),
        descriptor,
        release,
        target,
        status,
        is_reboot_required,
    })
}

fn recurse_package_set(
    store: &dyn PackageStore,
    package_candidate: &PackageCandidate,
    install_target: &[InstallTarget],
    repos: &HashMap<Url, LoadedRepository>,
    set: &mut HashMap<PackageKey, PackageCandidate>
) -> Result<(), PackageCandidateError> {
    package_candidate.target.dependencies.keys().try_fold((), |_, key| {
        let key = if !key.starts_with("https://") && !key.starts_with("http://") {
            store.find_package_by_id(key).map(|x| x.0)
                .ok_or_else(|| PackageCandidateError::UnresolvedId(key.to_string()))?
        } else {
            PackageKey::try_from(&**key).map_err(|_| PackageCandidateError::UnresolvedId(key.to_string()))?
        };

        if set.contains_key(&key) {
            return Ok(());
        }

        let candidate = resolve_package_candidate(store, &key, install_target, repos)?;
        set.insert(key, candidate);
        Ok(())
    })
}

pub(crate) fn resolve_package_set(
    store: &dyn PackageStore,
    install_candidates: &[PackageKey],
    install_target: &[InstallTarget],
) -> Result<Vec<PackageCandidate>, PackageCandidateError> {
    let repos = store.repos();
    let repos = repos.read().unwrap();

    // Resolve initial package set
    let mut candidate_set = install_candidates.iter().map(|key| {
        resolve_package_candidate(store, &key, install_target, &*repos).map(|v| (key.to_owned(), v))
    }).collect::<Result<HashMap<_, _>, _>>()?;

    // Iterate all dependencies until we achieve victory
    let values = candidate_set.values().cloned().collect::<Vec<_>>();

    values.iter().try_fold((), |_, candidate| {
        recurse_package_set(store, candidate, install_target, &*repos, &mut candidate_set)
    })?;

    // Take our candidate set and resolve it down to a mutation set
    Ok(candidate_set
        .into_iter()
        .filter_map(|(key, candidate)| {
            if candidate.status == PackageStatus::UpToDate {
                None
            } else {
                Some(candidate)
            }
        })
        .collect())
}
