use anyhow::{anyhow, Result};
use std::time::Duration;
use std::{
    ffi::{OsStr, OsString},
    path::Path,
};
use tokio::sync::mpsc;

use windows_service::{
    define_windows_service,
    service::{
        ServiceAccess, ServiceControl, ServiceControlAccept, ServiceErrorControl, ServiceExitCode,
        ServiceInfo, ServiceStartType, ServiceState, ServiceStatus, ServiceType,
    },
    service_control_handler::{self, ServiceControlHandlerResult},
    service_dispatcher,
    service_manager::{ServiceManager, ServiceManagerAccess},
};

pub const SERVICE_NAME: &str = "pahkat-server";
const SERVICE_TYPE: ServiceType = ServiceType::OWN_PROCESS;
const SERVICE_DISPLAY_NAME: &str = "Pahkat Service";
const NAMED_PIPE: &str = "//./pipe/pahkat";

pub fn install_service(exe_path: &Path) -> Result<()> {
    let manager_access = ServiceManagerAccess::CONNECT | ServiceManagerAccess::CREATE_SERVICE;
    let service_manager = ServiceManager::local_computer(None::<&str>, manager_access)?;

    let service_info = ServiceInfo {
        name: OsString::from(SERVICE_NAME),
        display_name: OsString::from(SERVICE_DISPLAY_NAME),
        service_type: SERVICE_TYPE,
        start_type: ServiceStartType::AutoStart,
        error_control: ServiceErrorControl::Normal,
        executable_path: exe_path.to_path_buf(),
        launch_arguments: vec![OsString::from("service"), OsString::from("run")],
        dependencies: vec![],
        account_name: None,
        account_password: None,
    };
    let _service = service_manager.create_service(&service_info, ServiceAccess::empty())?;

    Ok(())
}

pub fn uninstall_service() -> Result<()> {
    let manager_access = ServiceManagerAccess::CONNECT;
    let service_manager = ServiceManager::local_computer(None::<&str>, manager_access)?;

    let service_access = ServiceAccess::QUERY_STATUS | ServiceAccess::DELETE;

    match service_manager.open_service(OsString::from(SERVICE_NAME), service_access) {
        Err(e) => {
            dbg!(e);
            Ok(())
        }
        Ok(service) => Ok(service.delete()?),
    }
}

pub async fn stop_service() -> Result<()> {
    let manager_access = ServiceManagerAccess::CONNECT;
    let service_manager = ServiceManager::local_computer(None::<&str>, manager_access)?;

    let service_access = ServiceAccess::QUERY_STATUS | ServiceAccess::STOP;
    let service = service_manager.open_service(OsString::from(SERVICE_NAME), service_access)?;

    let service_status = service.query_status()?;
    while service_status.current_state != ServiceState::Stopped {
        service.stop()?;
        // Wait for service to stop
        tokio::time::delay_for(Duration::from_secs(1));
    }

    Ok(())
}

pub async fn start_service() -> Result<()> {
    let manager_access = ServiceManagerAccess::CONNECT;
    let service_manager = ServiceManager::local_computer(None::<&str>, manager_access)?;

    let service_access = ServiceAccess::QUERY_STATUS | ServiceAccess::START;
    let service = service_manager.open_service(OsString::from(SERVICE_NAME), service_access)?;

    loop {
        let mut service_status = service.query_status()?;
        if service_status.current_state == ServiceState::Running
            || service_status.current_state == ServiceState::StartPending
        {
            break;
        }

        if let Err(e) = service.start(&[OsStr::new("shitty types")]) {
            dbg!(e);
            break;
        }

        // Wait for service to start
        tokio::time::delay_for(Duration::from_secs(1));
    }

    Ok(())
}

pub fn run_service() -> Result<()> {
    service_dispatcher::start(SERVICE_NAME, ffi_service_main)?;
    Ok(())
}

define_windows_service!(ffi_service_main, service_main);

fn service_main(_: Vec<OsString>) {
    use tokio::runtime::Runtime;

    // winlog::register(SERVICE_DISPLAY_NAME);
    // winlog::init(SERVICE_DISPLAY_NAME).ok();
    use flexi_logger::{opt_format, Logger};
    let config_dir = pahkat_client::defaults::config_path().unwrap();
    let service_logs = config_dir.join("logs");
    Logger::with_str("trace")
        .log_to_file()
        .directory(service_logs)
        .format(opt_format)
        .start()
        .unwrap();

    log::debug!("logging initialized");
    // Create the runtime
    let mut rt = Runtime::new().unwrap();
    // Execute the future, blocking the current thread until completion
    rt.block_on(async {
        service_runner().await.unwrap();
    });
}

async fn service_runner() -> Result<()> {
    // shutdown channel & event handler to shut down service
    let (mut shutdown_tx, mut shutdown_rx) = mpsc::unbounded_channel();

    let event_handler = move |control_event| -> ServiceControlHandlerResult {
        match control_event {
            ServiceControl::Interrogate => ServiceControlHandlerResult::NoError,

            ServiceControl::Stop => {
                shutdown_tx.send(()).unwrap();
                ServiceControlHandlerResult::NoError
            }

            _ => ServiceControlHandlerResult::NotImplemented,
        }
    };

    let status_handle = service_control_handler::register(SERVICE_NAME, event_handler)?;

    // Report service as running
    status_handle.set_service_status(ServiceStatus {
        service_type: SERVICE_TYPE,
        current_state: ServiceState::Running,
        controls_accepted: ServiceControlAccept::STOP,
        exit_code: ServiceExitCode::Win32(0),
        checkpoint: 0,
        wait_hint: Duration::default(),
    })?;

    log::debug!("i'm running");
    crate::start(Path::new(NAMED_PIPE), None, shutdown_rx).await?;
    log::info!("Shutting down");

    // Tell the system that service has stopped.
    status_handle.set_service_status(ServiceStatus {
        service_type: SERVICE_TYPE,
        current_state: ServiceState::Stopped,
        controls_accepted: ServiceControlAccept::empty(),
        exit_code: ServiceExitCode::Win32(0),
        checkpoint: 0,
        wait_hint: Duration::default(),
    })?;

    Ok(())
}
