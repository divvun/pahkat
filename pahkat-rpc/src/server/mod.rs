pub mod selfupdate;
pub mod watch;
#[cfg(windows)]
pub mod windows;

use anyhow::Result;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("IO error")]
    Io(#[from] std::io::Error),

    #[error("Path error")]
    Path(#[from] pathos::Error),

    #[error("Set logger error")]
    SetLoggerError(#[from] log::SetLoggerError),
}

#[cfg(unix)]
fn fix_path_perms(path: &std::path::Path) -> Result<(), std::io::Error> {
    use std::{fs::Permissions, os::unix::fs::PermissionsExt};

    let meta = std::fs::metadata(path)?;
    let mut perms = meta.permissions();
    perms.set_mode(0o755);
    std::fs::set_permissions(path, perms)?;
    Ok(())
}

#[cfg(not(unix))]
fn fix_path_perms(_path: &std::path::Path) -> Result<(), std::io::Error> {
    Ok(())
}

pub fn setup_logger(name: &str) -> Result<(), Error> {
    let log_path = pahkat_client::defaults::log_path()?;
    std::fs::create_dir_all(&log_path)?;
    fix_path_perms(&log_path)?;

    fern::Dispatch::new()
        .format(|out, message, record| {
            out.finish(format_args!(
                "[{} {:<5} {}] {}",
                chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true),
                record.level(),
                record.target(),
                message
            ))
        })
        .level(log::LevelFilter::Info)
        .level_for("pahkat_rpc", log::LevelFilter::Debug)
        .level_for("pahkat_client", log::LevelFilter::Debug)
        .chain(std::io::stdout())
        .chain(fern::log_file(log_path.join(format!("{}.log", name)))?)
        .apply()?;

    log::debug!("logging initialized");
    log::debug!("Log path: {}", log_path.display());
    Ok(())
}
