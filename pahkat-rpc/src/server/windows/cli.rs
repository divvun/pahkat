use super::service;
use anyhow::Result;

use std::{path::PathBuf, time::Duration};
use structopt::StructOpt;

#[derive(Debug, StructOpt)]
pub enum ServiceOpts {
    Install,
    Uninstall,
    Stop,
    Run,
    SelfUpdate {
        #[structopt(long)]
        service_executable: PathBuf,
    },
}

pub async fn run_service_command(opts: &ServiceOpts) -> Result<()> {
    match opts {
        ServiceOpts::Install => {
            crate::server::setup_logger("self-update").unwrap_or(());
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
                tokio::time::sleep(Duration::from_secs(1)).await;
                if let Err(e) = service::install_service(&exe_path) {
                    if retries <= 0 {
                        log::error!("Failed to install service: {:?}", e);
                        return Err(e);
                    }
                    retries -= 1;
                    log::error!("Failed to install service, retrying: {:?}", e);
                } else {
                    break;
                }
            }
            service::start_service().await?;
            println!("Successfully installed service");
        }
        ServiceOpts::Uninstall => {
            crate::server::setup_logger("self-update").unwrap_or(());
            println!("Stopping service {}", service::SERVICE_NAME);
            service::stop_service().await?;
            println!("Uninstalling service {}", service::SERVICE_NAME);
            service::uninstall_service().await?;
            println!("Successfully uninstalled service {}", service::SERVICE_NAME);
        }
        ServiceOpts::Stop => {
            crate::server::setup_logger("self-update").unwrap_or(());
            println!("Stopping service {}", service::SERVICE_NAME);
            service::stop_service().await?;
        }
        ServiceOpts::Run => {
            println!("running service!");
            service::run_service()?;
        }
        ServiceOpts::SelfUpdate { service_executable } => {
            crate::server::setup_logger("self-update").unwrap_or(());
            super::self_update(&service_executable).await?;
        }
    };

    Ok(())
}
