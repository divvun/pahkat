use std::ffi::OsString;
use std::time::Duration;
use std::path::Path;
use anyhow::{Error, Result};
    
use windows_service::{
    define_windows_service,
    service::{
        ServiceAccess, ServiceInfo, ServiceControl, ServiceErrorControl, ServiceControlAccept, ServiceExitCode, ServiceState, ServiceStatus,
        ServiceType, ServiceStartType,
    },
    service_control_handler::{self, ServiceControlHandlerResult},
    service_manager::{ServiceManager, ServiceManagerAccess},
    service_dispatcher,
};
use tokio::runtime::Runtime;
use tokio::sync::mpsc;

const SERVICE_NAME: &str = "pahkat_service";
const SERVICE_DISPLAY_NAME: &str = "Pahkat Service";
const SERVICE_TYPE: ServiceType = ServiceType::OWN_PROCESS;
const NAMED_PIPE: &str = "//./pipe/pahkat";

define_windows_service!(ffi_run_service, run_service);

fn run_service(arguments: Vec<OsString>) {
    if let Err(e) = run_service_inner() {
        log::error!("{:?}", e);
    }
}

fn run_service_inner() -> Result<()> {
    log::info!("Registering logging services...");
    winlog::register(SERVICE_DISPLAY_NAME);
    winlog::init(SERVICE_DISPLAY_NAME)?;

    log::info!("Starting runtime...");
    let mut rt = Runtime::new().unwrap();
    let (shutdown_tx, shutdown_rx) = mpsc::unbounded_channel();

    // The entry point where execution will start on a background thread after a call to
    // `service_dispatcher::start` from `main`.

    let event_handler = move |control_event| -> ServiceControlHandlerResult {
        match control_event {
            // Notifies a service to report its current status information to the service
            // control manager. Always return NoError even if not implemented.
            ServiceControl::Interrogate => ServiceControlHandlerResult::NoError,

            // Handle stop
            ServiceControl::Stop => {
                shutdown_tx.send(()).unwrap();
                ServiceControlHandlerResult::NoError
            }

            _ => ServiceControlHandlerResult::NotImplemented,
        }
    };

    log::info!("Registering status handler...");
    // Register system service event handler.
    // The returned status handle should be used to report service status changes to the system.
    let status_handle = service_control_handler::register(SERVICE_NAME, event_handler)?;
    
    log::info!("Settings service status...");
    // Tell the system that service is running
    status_handle.set_service_status(ServiceStatus {
        service_type: SERVICE_TYPE,
        current_state: ServiceState::Running,
        controls_accepted: ServiceControlAccept::STOP,
        exit_code: ServiceExitCode::Win32(0),
        checkpoint: 0,
        wait_hint: Duration::default(),
    })?;

    log::info!("Running service!");
    let result = rt.block_on(pahkat_rpc::start(Path::new(NAMED_PIPE), None, shutdown_rx));

    log::info!("Service finished! {:?}", &result);

    // Tell the system that service has stopped.
    status_handle.set_service_status(ServiceStatus {
        service_type: SERVICE_TYPE,
        current_state: ServiceState::Stopped,
        controls_accepted: ServiceControlAccept::empty(),
        exit_code: ServiceExitCode::Win32(0),
        checkpoint: 0,
        wait_hint: Duration::default(),
    })?;

    result
}

use structopt::StructOpt;

#[derive(Debug, Copy, Clone, StructOpt)]
enum Command {
    Install,
    Uninstall,
    Start,
}

impl Default for Command {
    fn default() -> Self {
        Command::Start
    }
}

#[derive(Debug, StructOpt)]
struct Args {
    #[structopt(subcommand)]
    command: Option<Command>,
}

fn install() -> windows_service::Result<()> {
    let manager_access = ServiceManagerAccess::CONNECT | ServiceManagerAccess::CREATE_SERVICE;
    let service_manager = ServiceManager::local_computer(None::<&str>, manager_access)?;

    // This example installs the service defined in `examples/ping_service.rs`.
    // In the real world code you would set the executable path to point to your own binary
    // that implements windows service.
    let service_binary_path = std::env::current_exe().unwrap();
    println!("Installing binary: {}", service_binary_path.display());

    let service_info = ServiceInfo {
        name: OsString::from(SERVICE_NAME),
        display_name: OsString::from(SERVICE_DISPLAY_NAME),
        service_type: ServiceType::OWN_PROCESS,
        start_type: ServiceStartType::AutoStart,
        error_control: ServiceErrorControl::Normal,
        executable_path: service_binary_path,
        launch_arguments: vec![],
        dependencies: vec![],
        account_name: None, // run as System
        account_password: None,
    };
    
    let _service = service_manager.create_service(&service_info, ServiceAccess::empty())?;
    
    Ok(())
}

fn uninstall() -> windows_service::Result<()> {
    let manager_access = ServiceManagerAccess::CONNECT;
    let service_manager = ServiceManager::local_computer(None::<&str>, manager_access)?;

    let service_access = ServiceAccess::QUERY_STATUS | ServiceAccess::STOP | ServiceAccess::DELETE;
    let service = service_manager.open_service(SERVICE_NAME, service_access)?;

    let service_status = service.query_status()?;
    if service_status.current_state != ServiceState::Stopped {
        service.stop()?;
        // Wait for service to stop
        std::thread::sleep(Duration::from_secs(1));
    }

    service.delete()?;
    Ok(())
}

fn main() -> windows_service::Result<()> {
    let args = Args::from_args();
    let command = args.command.unwrap_or(Command::Start);

    match command {
        Command::Start => {
            log::info!("Calling service dispatcher...");
            service_dispatcher::start(SERVICE_NAME, ffi_run_service)
        },
        Command::Install => install(),
        Command::Uninstall => uninstall(),
    }
}