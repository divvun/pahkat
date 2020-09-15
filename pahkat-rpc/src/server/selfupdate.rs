use futures::stream::{StreamExt, TryStreamExt};
use once_cell::sync::Lazy;
use pahkat_client::package_store::DownloadEvent;
use pahkat_client::types::repo::RepoUrl;
use pahkat_client::{
    config::RepoRecord, package_store::InstallTarget, PackageAction, PackageActionType, PackageKey,
    PackageStatus, PackageStore, PackageTransaction,
};
use std::convert::TryFrom;
use std::error::Error;

use pahkat_client::Config;

pub const REPO: Lazy<RepoUrl> =
    Lazy::new(|| "https://pahkat.uit.no/divvun-installer/".parse().unwrap());
pub const REPO_CHANNEL: Lazy<Option<&'static str>> = Lazy::new(|| {
    let channel = env!("CHANNEL");
    if channel.trim() == "" {
        None
    } else {
        Some(channel)
    }
});

pub const SELFUPDATE_KEYS: Lazy<Vec<PackageKey>> = Lazy::new(|| {
    vec![
        PackageKey::try_from("https://pahkat.uit.no/divvun-installer/packages/divvun-installer")
            .unwrap(),
        #[cfg(windows)]
        PackageKey::try_from("https://pahkat.uit.no/divvun-installer/packages/pahkat-service")
            .unwrap(),
    ]
});

fn make_config() -> Config {
    let mut config = Config::read_only();
    config
        .repos_mut()
        .insert(
            REPO.to_owned(),
            RepoRecord {
                channel: REPO_CHANNEL.as_ref().map(|x| x.to_string()),
            },
        )
        .unwrap();

    log::trace!("Creating self-update config:");
    log::trace!("{:#?}", &config);

    config
}

#[cfg(feature = "windows")]
#[inline]
pub(crate) async fn package_store() -> Box<dyn PackageStore> {
    let config = make_config();
    Box::new(pahkat_client::WindowsPackageStore::new(config).await)
}

#[cfg(feature = "macos")]
#[inline]
pub(crate) async fn package_store() -> Box<dyn PackageStore> {
    let config = make_config();
    Box::new(pahkat_client::MacOSPackageStore::new(config).await)
}

pub(crate) fn requires_update(store: &dyn PackageStore) -> bool {
    let mut is_requiring_update = false;

    for key in &*SELFUPDATE_KEYS {
        let status = store.status(&key, InstallTarget::System);

        log::trace!("requires_update store.status: {:?}", status);

        let is_requiring_update = match status {
            Ok(status) => match status {
                PackageStatus::NotInstalled => {
                    log::error!(
                        "Self-update detected that Pahkat Service was not installed at all?"
                    );
                    // false
                }
                PackageStatus::RequiresUpdate => {
                    is_requiring_update = true;
                }
                PackageStatus::UpToDate => {}
            },
            Err(err) => {
                log::error!("{:?}", err);
            }
        };
    }

    is_requiring_update
}

#[cfg(windows)]
pub async fn install(store: &dyn PackageStore) -> Result<(), Box<dyn Error>> {
    super::windows::initiate_self_update()?;
    // Wait some time for the impending shutdown
    tokio::time::delay_for(std::time::Duration::from_secs(10)).await;
    Ok(())
}

#[cfg(all(feature = "macos", not(feature = "launchd")))]
pub async fn install(store: &dyn PackageStore) -> Result<(), Box<dyn Error>> {
    log::info!("No doing anything, in test mode.");
    Ok(())
}

#[cfg(feature = "launchd")]
pub async fn install(store: &dyn PackageStore) -> Result<(), Box<dyn Error>> {
    for key in &*super::selfupdate::SELFUPDATE_KEYS {
        if let Ok(PackageStatus::RequiresUpdate) = store.status(key, InstallTarget::System) {
            // Expect the package to be downloaded already
            match store.install(key, InstallTarget::System) {
                Ok(_) => {
                    log::info!("Self-updated successfully.");
                }
                Err(e) => {
                    log::error!("Error during self-update installation: {:?}", e);
                    return Err(e.into());
                }
            }
        }
    }

    // Stop should trigger an immediate restart.
    std::process::Command::new("launchctl")
        .args(&["stop", "no.divvun.pahkatd"])
        .spawn()?;
    Ok(())
}

pub(crate) async fn self_update() -> Result<bool, Box<dyn Error>> {
    log::debug!("Getting self-update store...");
    let store = package_store().await;

    if !requires_update(&*store) {
        log::debug!("No update required, self-updater finishing.");
        return Ok(false);
    }

    // Retry 5 times
    let retries = 5i32;
    'downloader: for i in 1i32..=retries {
        log::debug!("Attempt {} of self update...", i);

        // If update is available, download it.
        log::debug!("Downloading self-update package...");
        for key in &*SELFUPDATE_KEYS {
            let mut stream = store.download(&key);

            while let Some(result) = stream.next().await {
                match result {
                    DownloadEvent::Progress((current, total)) => {
                        log::debug!("Downloaded: {}/{}", current, total)
                    }
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
                        break 'downloader;
                    }
                }
            }
        }
    }

    install(&*store).await?;

    Ok(true)
}
