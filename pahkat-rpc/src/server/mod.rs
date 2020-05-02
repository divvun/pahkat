pub mod cli;
#[cfg(windows)]
pub mod windows;

use anyhow::{anyhow, Result};
use lazy_static::lazy_static;
use log::{info, warn};
use pahkat_client::{
    config::RepoRecord, package_store::SharedStoreConfig, PackageKey, PackageStatus, PackageStore,
};
use std::convert::TryFrom;
use std::sync::Arc;
use std::time::Duration;
use tokio::time;

lazy_static! {
    pub static ref UPDATER_KEY: PackageKey =
        PackageKey::try_from("https://x.brendan.so/divvun-pahkat-repo/packages/windivvun").unwrap();
    pub static ref UPDATER_DEFAULT_CHANNEL: Option<String> = Some("nightly".to_string());
}

/// Ensure the repository contains the PackageKey for the Pahkat service, potentially inserting it
pub fn ensure_pahkat_service_key(config: SharedStoreConfig) -> (bool, PackageKey) {
    let updater_key = UPDATER_KEY.clone();
    if let Some(_) = config
        .read()
        .unwrap()
        .repos()
        .get(&updater_key.repository_url)
    {
        return (false, updater_key);
    }

    config
        .write()
        .unwrap()
        .repos_mut()
        .insert(
            updater_key.repository_url.clone(),
            RepoRecord {
                channel: UPDATER_DEFAULT_CHANNEL.clone(),
            },
        )
        .unwrap();
    return (true, updater_key);
}

#[derive(Eq, PartialEq)]
pub enum SelfUpdateStatus {
    Recheck,
    Required,
    UpToDate,
}

pub fn check_for_self_update(store: Arc<dyn PackageStore>) -> Result<SelfUpdateStatus> {
    let (modified, pakhat_service_key) = ensure_pahkat_service_key(store.config());
    if modified {
        return Ok(SelfUpdateStatus::Required);
    }

    info!("checking for self update");

    match store.status(&pakhat_service_key, pahkat_client::InstallTarget::System) {
        Ok(PackageStatus::RequiresUpdate) => Ok(SelfUpdateStatus::Required),
        Ok(status) => {
            info!("self update status: {:?}", status);
            Ok(SelfUpdateStatus::UpToDate)
        }
        Err(e) => {
            warn!("self update check failed: {:?}", e);
            Err(anyhow!(e))
        }
    }
}

pub async fn check_and_initiate_self_update(store: Arc<dyn PackageStore>) -> Result<bool> {
    if check_for_self_update(store)? == SelfUpdateStatus::Required {
        #[cfg(windows)]
        {
            info!("self update required, initiating");
            windows::initiate_self_update()?;
            // Wait some time for the impending shutdown
            time::delay_for(Duration::from_secs(10)).await;
            // Skip normal update check
            Ok(true)
        }
        #[cfg(not(windows))]
        {
            warn!("self update not implemented");
            Ok(false)
        }
    } else {
        Ok(false)
    }
}
