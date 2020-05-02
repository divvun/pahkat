pub mod cli;
pub mod service;

use anyhow::Result;
use pahkat_client::{config::RepoRecord, PackageKey};
use std::process::Command;

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
