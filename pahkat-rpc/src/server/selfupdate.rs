use pahkat_client::{
    config::RepoRecord, package_store::InstallTarget, PackageAction, PackageActionType, PackageKey,
    PackageStatus, PackageStore, PackageTransaction,
};
use std::error::Error;
use futures::stream::{StreamExt, TryStreamExt};
use once_cell::sync::Lazy;
use pahkat_client::package_store::DownloadEvent;

use std::convert::TryFrom;

pub const UPDATER_DEFAULT_CHANNEL: &str = "nightly";
pub static UPDATER_KEY: Lazy<PackageKey> = Lazy::new(||
    PackageKey::try_from("https://pahkat.uit.no/divvun-installer/packages/divvun-installer").unwrap());

#[cfg(feature = "windows")]
async fn package_store() -> Box<dyn PackageStore> {
    let mut config = Config::read_only();
    config.repos_mut().insert(UPDATER_KEY.repository_url.clone(), RepoRecord {
        channel: Some(UPDATER_DEFAULT_CHANNEL.to_string())
    }).unwrap();

    Box::new(WindowsPackageStore::new(config).await)
}

#[cfg(feature = "macos")]
async fn package_store() -> Box<dyn PackageStore> {
    use pahkat_client::{Config, MacOSPackageStore};
    let mut config = Config::read_only();

    config.repos_mut().insert(UPDATER_KEY.repository_url.clone(), RepoRecord {
        channel: Some(UPDATER_DEFAULT_CHANNEL.to_string())
    }).unwrap();

    Box::new(MacOSPackageStore::new(config).await)
}

fn requires_update(store: &dyn PackageStore) -> bool {
    let is_requiring_update = match store.status(&UPDATER_KEY, InstallTarget::System) {
        Ok(status) => match status {
            PackageStatus::NotInstalled => {
                log::error!("Self-update detected that Pahkat Service was not installed at all?");
                false
            }
            PackageStatus::RequiresUpdate => true,
            PackageStatus::UpToDate => false,
        }
        Err(err) => {
            log::error!("{:?}", err);
            false
        }
    };

    is_requiring_update
}

#[cfg(windows)]
pub async fn install(store: &dyn PackageStore) -> Result<(), Box<dyn Error>> {
    windows::initiate_self_update()?;
    // Wait some time for the impending shutdown
    time::delay_for(Duration::from_secs(10)).await;
    Ok(())
}

#[cfg(feature = "launchd")]
pub async fn install(store: &dyn PackageStore) -> Result<(), Box<dyn Error>> {
    store.install(&UPDATER_KEY, InstallTarget::System)?;

    // Stop should trigger an immediate restart.
    std::process::Command::new("launchctl").args(&["stop", "no.divvun.pahkatd"]).spawn()?;
    Ok(())
}

pub(crate) async fn self_update() -> Result<bool, Box<dyn Error>> {
    let store = package_store().await;

    if !requires_update(&*store) {
        return Ok(false);
    }

    // Retry 5 times
    let retries = 5i32;
    'downloader: for i in 1i32..=retries {
        // If update is available, download it.
        let mut stream = store.download(&UPDATER_KEY);

        while let Some(result) = stream.next().await {
            match result {
                DownloadEvent::Progress((current, total)) => log::debug!("Downloaded: {}/{}", current, total),
                DownloadEvent::Error(error) => {
                    log::error!("Error downloading update: {:?}", error);
                    if i == retries {
                        log::error!("Downloading failed {} times; aborting!", retries);
                        return Ok(false);
                    }
                    tokio::time::delay_for(std::time::Duration::from_secs(2)).await;
                    continue 'downloader;
                }
                DownloadEvent::Complete(_) => {
                    log::debug!("Download completed!");
                    break;
                }
            }
        }
    }

    install(&*store).await?;

    Ok(true)
}