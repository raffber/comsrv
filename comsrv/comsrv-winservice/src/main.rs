#[macro_use]
extern crate windows_service;

use std::ffi::OsString;
use windows_service::{Result, service_dispatcher, service_control_handler};
use windows_service::service_control_handler::ServiceControlHandlerResult;
use windows_service::service::{ServiceControl, ServiceExitCode, ServiceControlAccept, ServiceState, ServiceType, ServiceStatus};
use comsrv::app::App;
use tokio::runtime::Runtime;
use std::time::Duration;
use tokio::task;

define_windows_service!(ffi_service_main, my_service_main);

const SERVICE_NAME: &'static str = "comsrv";


fn my_service_main(arguments: Vec<OsString>) {
    if let Err(_e) = run_service(arguments) {
        // Handle error in some way.
    }
}

fn run_service(arguments: Vec<OsString>) -> windows_service::Result<()> {
    let (app, rx) = App::new();
    let app2 = app.clone();
    let event_handler = move |control_event| -> ServiceControlHandlerResult {
        match control_event {
            ServiceControl::Stop => {
                task::spawn(app2.clone().shutdown());
                ServiceControlHandlerResult::NoError
            }
            // All services must accept Interrogate even if it's a no-op.
            ServiceControl::Interrogate => ServiceControlHandlerResult::NoError,
            _ => ServiceControlHandlerResult::NotImplemented,
        }
    };
    let status_handle = service_control_handler::register(SERVICE_NAME, event_handler)?;
    let next_status = ServiceStatus {
        // Should match the one from system service registry
        service_type: ServiceType::OWN_PROCESS,
        // The new state
        current_state: ServiceState::Running,
        // Accept stop events when running
        controls_accepted: ServiceControlAccept::STOP,
        // Used to report an error when starting or stopping only, otherwise must be zero
        exit_code: ServiceExitCode::Win32(0),
        // Only used for pending states, otherwise must be zero
        checkpoint: 0,
        // Only used for pending states, otherwise must be zero
        wait_hint: Duration::default(),
        // Unused for setting status
        process_id: None,
    };

    // Tell the system that the service is running now
    status_handle.set_service_status(next_status)?;

    let mut rt = Runtime::new().unwrap();
    rt.block_on(app.run(rx));
    // Register system service event handler
    Ok(())
}

fn main() -> Result<()> {
    service_dispatcher::start(SERVICE_NAME, ffi_service_main)?;
    Ok(())
}