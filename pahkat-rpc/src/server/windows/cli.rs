use super::service;
use anyhow::{anyhow, Result};
use log::info;
use std::fs::OpenOptions;
use std::{
    path::{Path, PathBuf},
    time::Duration,
};
use structopt::StructOpt;

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
    service::stop_service().await?;
    info!("waiting for write access to executable");
    while let Err(e) = OpenOptions::new()
        .write(true)
        .create(true)
        .open(&service_executable)
    {
        info!("err {:?}", e);
        tokio::time::delay_for(Duration::from_secs(1)).await;
    }
    info!("do the update things");

    Ok(())
}

pub async fn run_service_command(opts: &ServiceOpts) -> Result<()> {
    match opts {
        ServiceOpts::Install => {
            let exe_path = std::env::current_exe()?;
            println!(
                "Installing service {} at {:?}",
                service::SERVICE_NAME,
                exe_path
            );

            service::stop_service().await.ok();
            service::uninstall_service().ok();
            if let Err(e) = service::install_service(&exe_path) {
                eprintln!("Failed to install service: {:?}", e);
                anyhow!(e);
            }

            service::start_service().await?;
        }
        ServiceOpts::Uninstall => {
            println!("Stopping service {}", service::SERVICE_NAME);
            service::stop_service().await?;
            println!("Uninstalling service {}", service::SERVICE_NAME);
            service::uninstall_service()?;
            println!("Uninstalled service {}", service::SERVICE_NAME);
        }
        ServiceOpts::Stop => {
            println!("Stopping service {}", service::SERVICE_NAME);
            service::stop_service().await?;
        }
        ServiceOpts::Run => {
            println!("running service!");
            service::run_service();
        }
        ServiceOpts::SelfUpdate { service_executable } => {
            super::setup_logger("self-update");
            info!("I'm a self updater!");
            self_update(&service_executable).await?;
        }
    }

    Ok(())
}
