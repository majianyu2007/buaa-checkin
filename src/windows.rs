#![cfg(windows)]

use std::ffi::OsString;
use std::time::Duration;
use windows_service::{
    define_windows_service,
    service::{
        ServiceControl, ServiceControlAccept, ServiceExitCode, ServiceState, ServiceStatus, ServiceType,
    },
    service_control_handler::{self, ServiceControlHandlerResult},
    service_dispatcher,
};
use tracing::{info, error};

const SERVICE_NAME: &str = "buaa-checkin";
const SERVICE_TYPE: ServiceType = ServiceType::OWN_PROCESS;

pub fn run_service() -> windows_service::Result<()> {
    service_dispatcher::start(SERVICE_NAME, ffi_service_main)
}

define_windows_service!(ffi_service_main, my_service_main);

pub fn my_service_main(_arguments: Vec<OsString>) {
    if let Err(e) = run_service_inner() {
        error!("Windows service failed: {}", e);
    }
}

fn run_service_inner() -> windows_service::Result<()> {
    let event_handler = move |control_event| -> ServiceControlHandlerResult {
        match control_event {
            ServiceControl::Stop => {
                // Handle stop event
                std::process::exit(0);
            }
            ServiceControl::Interrogate => ServiceControlHandlerResult::NoError,
            _ => ServiceControlHandlerResult::NotImplemented,
        }
    };

    let status_handle = service_control_handler::register(SERVICE_NAME, event_handler)?;

    status_handle.set_service_status(ServiceStatus {
        service_type: SERVICE_TYPE,
        current_state: ServiceState::Running,
        controls_accepted: ServiceControlAccept::STOP,
        exit_code: ServiceExitCode::Win32(0),
        checkpoint: 0,
        wait_hint: Duration::default(),
        process_id: None,
    })?;

    info!("Windows service running...");

    // Start the async runtime and block
    let rt = tokio::runtime::Runtime::new().unwrap();
    rt.block_on(async {
        crate::run_server(".\\data".to_string(), Some(3000)).await;
    });

    status_handle.set_service_status(ServiceStatus {
        service_type: SERVICE_TYPE,
        current_state: ServiceState::Stopped,
        controls_accepted: ServiceControlAccept::empty(),
        exit_code: ServiceExitCode::Win32(0),
        checkpoint: 0,
        wait_hint: Duration::default(),
        process_id: None,
    })?;

    Ok(())
}

pub fn install_service(exe_path: &str) -> std::io::Result<()> {
    std::process::Command::new("sc")
        .arg("create")
        .arg(SERVICE_NAME)
        .arg("binPath=")
        .arg(exe_path)
        .arg("start=")
        .arg("auto")
        .output()?;
    std::process::Command::new("sc")
        .arg("description")
        .arg(SERVICE_NAME)
        .arg("BUAA Auto Check-in System")
        .output()?;
    Ok(())
}

pub fn uninstall_service() -> std::io::Result<()> {
    std::process::Command::new("sc")
        .arg("stop")
        .arg(SERVICE_NAME)
        .output()?;
    std::process::Command::new("sc")
        .arg("delete")
        .arg(SERVICE_NAME)
        .output()?;
    Ok(())
}
