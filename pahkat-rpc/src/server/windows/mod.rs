pub mod cli;
pub mod service;

use anyhow::Result;
use std::fs::OpenOptions;
use std::process::Command;
use std::{
    path::{Path, PathBuf},
    time::Duration,
};

use pahkat_client::{
    config::RepoRecord, package_store::InstallTarget, Config, PackageAction, PackageActionType,
    PackageKey, PackageStatus, PackageStore, PackageTransaction,
};

const SELF_UPDATE_TIMEOUT: u64 = 30;
const UPDATER_FILE_NAME: &str = "pahkat-updater.exe";

pub fn setup_logger(name: &str) -> Result<(), fern::InitError> {
    if let Some(log_path) = pahkat_client::defaults::log_path() {
        std::fs::create_dir_all(&log_path)?;

        fern::Dispatch::new()
            .format(|out, message, record| {
                out.finish(format_args!(
                    "{}[{}][{}] {}",
                    chrono::Local::now().format("[%Y-%m-%d][%H:%M:%S]"),
                    record.target(),
                    record.level(),
                    message
                ))
            })
            .level(log::LevelFilter::Trace)
            .chain(std::io::stdout())
            .chain(fern::log_file(log_path.join(format!("{}.log", name)))?)
            .apply()?;
    } else {
        env_logger::init();
    }

    log::debug!("logging initialized");
    Ok(())
}

pub fn initiate_self_update() -> Result<()> {
    // Launch self update exe
    let exe_path = std::env::current_exe()?;

    let tmp_updater_exe = exe_path.with_file_name(UPDATER_FILE_NAME);

    std::fs::copy(&exe_path, &tmp_updater_exe)?;

    Command::new(tmp_updater_exe)
        .arg("service")
        .arg("self-update")
        .arg("--service-executable")
        .arg(exe_path)
        .spawn()?;

    Ok(())
}

pub(crate) async fn self_update(
    service_executable: &Path,
) -> std::result::Result<(), anyhow::Error> {
    log::info!("Running self-update");

    let store = super::selfupdate::package_store().await;
    let _ = store.refresh_repos().await;

    if !super::selfupdate::requires_update(&*store) {
        log::warn!("no update required");
        return Ok(());
    }

    log::info!("shutting down running service");
    if let Err(e) = service::stop_service().await {
        // Whatever, this fails often while the service is shutting down
        log::warn!("stop service error: {:?}", e);
    }

    log::info!("waiting for write access to executable");

    tokio::time::timeout(std::time::Duration::from_secs(SELF_UPDATE_TIMEOUT), async {
        while let Err(e) = OpenOptions::new()
            .write(true)
            .create(true)
            .open(&service_executable)
        {
            log::info!("err {:?}", e);
            tokio::time::delay_for(std::time::Duration::from_secs(1)).await;
        }
    })
    .await?;

    log::info!("Beginning update check");

    // Expect the package to be downloaded already
    match store.install(&super::selfupdate::UPDATER_KEY, InstallTarget::System) {
        Ok(_) => {
            log::info!("Self-updated successfully.");
        }
        Err(e) => {
            log::error!("Error during self-update installation: {:?}", e);
            return Err(e.into());
        }
    }

    Ok(())
}
