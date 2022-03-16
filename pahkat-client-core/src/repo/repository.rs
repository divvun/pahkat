use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::generated::pahkat as pahkat_fbs;
use pahkat_types::{repo::RepoUrl, PackageKey};

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
    // pub hash_id: String,
    // TODO: last update
}

#[derive(Debug, Clone)]
pub struct LoadedRepository {
    pub info: pahkat_types::repo::Index,
    pub packages: Box<[u8]>,
    pub meta: LoadedRepositoryMeta,
}

impl LoadedRepository {
    pub async fn from_cache_or_url(
        url: RepoUrl,
        channel: Option<String>,
        cache_dir: PathBuf,
    ) -> Result<LoadedRepository, RepoDownloadError> {
        Self::from_url(url, channel).await
    }

    async fn from_url(
        url: RepoUrl,
        channel: Option<String>,
    ) -> Result<LoadedRepository, RepoDownloadError> {
        const USER_AGENT: &str = concat!("pahkat-client/", env!("VERGEN_GIT_SEMVER_LIGHTWEIGHT"), " (", env!("VERGEN_CARGO_TARGET_TRIPLE"), ")");
        let (tx, rx) = tokio::sync::oneshot::channel();

        tokio::spawn(async move {
            let result = async move {
                let client = reqwest::Client::builder()
                    .user_agent(USER_AGENT)
                    .referer(false)
                    .redirect(reqwest::redirect::Policy::none())
                    .build()?;

                log::trace!("Loading repo: {} channel:{:?}", &url, &channel);

                let info = client
                    .get(&format!("{}/index.toml", url))
                    .send()
                    .await?
                    .text()
                    .await?;
                let info: pahkat_types::repo::Index = toml::from_str(&info)?;

                let packages = client
                    .get(&format!("{}/packages/index.bin", url))
                    .send()
                    .await?
                    .bytes()
                    .await?
                    .to_vec()
                    .into_boxed_slice();

                let repo = LoadedRepository {
                    info,
                    packages,
                    meta: LoadedRepositoryMeta {
                        channel,
                        // hash_id: "".into(),
                    },
                };

                log::trace!("Loaded.");
                Ok(repo)
            }
            .await;

            tx.send(result).unwrap();
        });

        rx.await.unwrap()
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

    pub fn package_key(&self, descriptor: &pahkat_types::package::Descriptor) -> PackageKey {
        PackageKey::new_unchecked(
            self.info.repository.url.to_owned(),
            descriptor.package.id.clone(),
            None,
        )
    }
}
