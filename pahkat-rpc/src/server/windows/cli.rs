use super::service;
use anyhow::{anyhow, Result};
use log::{error, info, warn};
use pahkat_client::{package_store::InstallTarget, PackageStore};
use std::fs::OpenOptions;
use std::process::Command;
use std::sync::Arc;
use std::{
    path::{Path, PathBuf},
    time::Duration,
};
use structopt::StructOpt;

const SELF_UPDATE_TIMEOUT: u64 = 30;

#[derive(Debug, StructOpt)]
pub enum ServiceOpts {
    //SelfUpdate,
    Install,
    Uninstall,
    Stop,
    Run,
    SelfUpdate {
        #[structopt(long)]
        service_executable: PathBuf,
    },
}

async fn self_update(service_executable: &Path) -> Result<()> {
    info!("shutting down running service");
    if let Err(e) = service::stop_service().await {
        // Whatever, this fails often while the service is shutting down
        warn!("stop service error: {:?}", e);
    }

    info!("waiting for write access to executable");

    tokio::time::timeout(Duration::from_secs(SELF_UPDATE_TIMEOUT), async {
        while let Err(e) = OpenOptions::new()
            .write(true)
            .create(true)
            .open(&service_executable)
        {
            info!("err {:?}", e);
            tokio::time::delay_for(Duration::from_secs(1)).await;
        }
    })
    .await?;

    info!("Beginning update check");

    let path = pahkat_client::defaults::config_path().unwrap();
    let config = pahkat_client::Config::load(path, pahkat_client::Permission::ReadOnly)?;
    let store = Arc::new(pahkat_client::WindowsPackageStore::new(config).await);

    let pahkat_service_key = super::ensure_pahkat_service_key(store.config());

    let _ = store.refresh_repos().await;

    let status = store.status(&pahkat_service_key, InstallTarget::System)?;
    info!("updater package status: {:?}", status);

    if status != pahkat_client::PackageStatus::RequiresUpdate {
        warn!("no update required");
        return Ok(());
    }

    // Expect the package to be downloaded already

    // let action = PackageAction::install(pahkat_service_key, InstallTarget::System);
    // let transaction = PackageTransaction::new(store.clone(), vec![action]).unwrap();
    // let (cancel, stream) = transaction.process();

    // futures::pin_mut!(stream);

    // while let Some(status) = stream.next().await {
    //     info!("status {:?}", status);
    // }

    let installer = PathBuf::from(r"E:\ttc\pahkat\Output\install.exe");
    let output = Command::new(installer)
        .args(&["/VERYSILENT", "/SP-", "/SUPPRESSMSGBOXES", "/NORESTART"])
        .output()
        .unwrap();

    if !output.status.success() {
        error!("self update failed!");
        error!("output: {:?}", output);
        return Err(anyhow!("Self update failed"));
    }

    info!("self update finished!");
    info!("output: {:?}", output);

    Ok(())
}

pub async fn run_service_command(opts: &ServiceOpts) -> Result<()> {
    match opts {
        ServiceOpts::Install => {
            super::setup_logger("self-update").unwrap();

            let exe_path = std::env::current_exe()?;
            println!(
                "Installing service {} at {:?}",
                service::SERVICE_NAME,
                exe_path
            );

            service::stop_service().await?;
            service::uninstall_service().await?;
            // Installing fails at times immediately after an uninstall, try a few more times,
            // if it continues failing, the service is locked, i.e. something else has it open
            // for example services.msc
            let mut retries: i32 = 5;
            loop {
                tokio::time::delay_for(Duration::from_secs(1)).await;
                if let Err(e) = service::install_service(&exe_path) {
                    if retries <= 0 {
                        eprintln!("Failed to install service: {:?}", e);
                        return Err(e);
                    }
                    retries -= 1;
                    eprintln!("Failed to install service, retrying: {:?}", e);
                } else {
                    break;
                }
            }
            service::start_service().await?;
            println!("Successfully installed service");
        }
        ServiceOpts::Uninstall => {
            super::setup_logger("self-update").unwrap();

            println!("Stopping service {}", service::SERVICE_NAME);
            service::stop_service().await?;
            println!("Uninstalling service {}", service::SERVICE_NAME);
            service::uninstall_service().await?;
            println!("Successfully uninstalled service {}", service::SERVICE_NAME);
        }
        ServiceOpts::Stop => {
            super::setup_logger("self-update").unwrap();
            println!("Stopping service {}", service::SERVICE_NAME);
            service::stop_service().await?;
        }
        ServiceOpts::Run => {
            println!("running service!");
            service::run_service()?;
        }
        ServiceOpts::SelfUpdate { service_executable } => {
            super::setup_logger("self-update").unwrap();
            info!("I'm a self updater!");
            self_update(&service_executable).await?;
        }
    };

    Ok(())
}
