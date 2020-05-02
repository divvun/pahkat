pub mod cli;
pub mod service;

use anyhow::Result;
use pahkat_client::{config::RepoRecord, package_store::SharedStoreConfig, PackageKey};
use std::process::Command;

const UPDATER_FILE_NAME: &str = "pahkat-updater.exe";

/// Ensure the repository contains the PackageKey for the Pahkat service, potentially inserting it
pub fn ensure_pahkat_service_key(config: SharedStoreConfig) -> PackageKey {
    let updater_key = super::UPDATER_KEY.clone();
    if let Some(_) = config
        .read()
        .unwrap()
        .repos()
        .get(&updater_key.repository_url)
    {
        return updater_key;
    }

    config
        .write()
        .unwrap()
        .repos_mut()
        .insert(
            updater_key.repository_url.clone(),
            RepoRecord {
                channel: super::UPDATER_DEFAULT_CHANNEL.clone(),
            },
        )
        .unwrap();
    return updater_key;
}

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
