pub mod cli;
pub mod service;

use anyhow::Result;
use std::process::Command;

const UPDATER_FILE_NAME: &str = "pahkat-updater.exe";

pub fn setup_logger(name: &str) -> Result<(), fern::InitError> {
    let config_dir = pahkat_client::defaults::config_path().unwrap();
    let service_logs = config_dir.join("logs").join(format!("{}.log", name));

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
        .chain(fern::log_file(service_logs)?)
        .apply()?;

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
