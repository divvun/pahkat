pub mod cli;
pub mod service;

use anyhow::{bail, Result};
use std::fs::OpenOptions;

use std::os::windows::io::RawHandle;
use std::process::Command;
use std::{
    path::{Path},
};
use winapi::um::errhandlingapi::GetLastError;
use winapi::um::namedpipeapi::ImpersonateNamedPipeClient;
use winapi::um::securitybaseapi::RevertToSelf;
use windows_accesstoken::information::TokenElevation;
use windows_accesstoken::AccessToken;
use windows_accesstoken::TokenAccessLevel;
use windows_accesstoken::{
    information::{Groups, LinkedToken},
    security::{GroupSidAttributes, SecurityIdentifier, WellKnownSid},
};

use pahkat_client::{
    package_store::InstallTarget, PackageStatus, PackageStore,
};

const SELF_UPDATE_TIMEOUT: u64 = 30;
const UPDATER_FILE_NAME: &str = "pahkat-updater.exe";

// use super::setup

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
            tokio::time::sleep(std::time::Duration::from_secs(1)).await;
        }
    })
    .await?;

    log::info!("Beginning update check");

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

    Ok(())
}

#[derive(Copy, Clone)]
pub struct HandleHolder(pub RawHandle);

unsafe impl Send for HandleHolder {}

pub fn is_connected_user_admin(handle: HandleHolder) -> Result<bool> {
    unsafe {
        log::trace!("about to impersonate with handle {:?}", handle.0);
        if ImpersonateNamedPipeClient(handle.0) == 0 {
            bail!("Error Impersonating client: {}", GetLastError())
        }
    }

    log::trace!("opening thread token");
    let token = AccessToken::open_thread(true, TokenAccessLevel::Query)?;

    if is_admin_token(&token)? {
        log::trace!("reverting to self");
        unsafe {
            RevertToSelf();
        }
        return Ok(true);
    }

    if token.token_information::<TokenElevation>()? == Some(TokenElevation::Limited) {
        log::trace!("getting linked token");

        let token = token.token_information::<LinkedToken>()?;

        if let Some(token) = token {
            if is_admin_token(&token)? {
                log::trace!("reverting to self");
                unsafe {
                    RevertToSelf();
                }
                return Ok(true);
            }
        }
    }

    unsafe {
        RevertToSelf();
    }
    Ok(false)
}

fn is_admin_token(token: &AccessToken) -> Result<bool, std::io::Error> {
    let admin_sid = SecurityIdentifier::from_known(WellKnownSid::WinBuiltinAdministratorsSid)?;

    log::trace!("getting token groups");
    if let Some(groups) = token.token_information::<Groups>()? {
        for group in groups {
            if group.0 == admin_sid && group.1.contains(GroupSidAttributes::SE_GROUP_ENABLED) {
                return Ok(true);
            }
        }
    }

    return Ok(false);
}
