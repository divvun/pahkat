pub mod cli;
#[cfg(windows)]
pub mod windows;

use anyhow::{anyhow, Result};
use lazy_static::lazy_static;
use log::{info, warn};
use pahkat_client::{PackageKey, PackageStatus, PackageStore};
use std::convert::TryFrom;
use std::sync::Arc;
use std::time::Duration;
use tokio::time;

lazy_static! {
    pub static ref UPDATER_KEY: PackageKey =
        PackageKey::try_from("https://x.brendan.so/divvun-pahkat-repo/packages/windivvun").unwrap();
    pub static ref UPDATER_DEFAULT_CHANNEL: Option<String> = Some("nightly".to_string());
}

pub async fn check_and_initiate_self_update(store: Arc<dyn PackageStore>) -> Result<bool> {
    #[cfg(windows)]
    {
        info!("checking for self update");
        let pakhat_service_key = windows::ensure_pahkat_service_key(store.config());
        match store.status(&pakhat_service_key, pahkat_client::InstallTarget::System) {
            Ok(PackageStatus::RequiresUpdate) => {
                info!("self update required, initiating");
                windows::initiate_self_update()?;
                // Wait some time for the impending shutdown
                time::delay_for(Duration::from_secs(10)).await;
                // Skip normal update check
                Ok(true)
            }
            Ok(status) => {
                info!("self update status: {:?}", status);
                Ok(false)
            }
            Err(e) => {
                warn!("self update check failed: {:?}", e);
                Err(anyhow!(e))
            }
        }
    }
    #[cfg(not(windows))]
    {
        warn!("self update not implemented");
        Ok(())
    }
}
