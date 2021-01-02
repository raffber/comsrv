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
use std::net::SocketAddr;

define_windows_service!(ffi_service_main, service_main);

const SERVICE_NAME: &'static str = "comsrv";


fn service_main(arguments: Vec<OsString>) {
    if let Err(_e) = run_service(arguments) {
        // Handle error in some way.
    }
}

fn run_service(_: Vec<OsString>) -> windows_service::Result<()> {
    let (app, rx) = App::new();
    let app2 = app.clone();
    let event_handler = move |control_event| -> ServiceControlHandlerResult {
        match control_event {
            ServiceControl::Stop => {
                let app3 = app2.clone();
                task::spawn(async move {
                    app3.shutdown().await;
                });
                ServiceControlHandlerResult::NoError
            }
            // All services must accept Interrogate even if it's a no-op.
            ServiceControl::Interrogate => ServiceControlHandlerResult::NoError,
            _ => ServiceControlHandlerResult::NotImplemented,
        }
    };
    let status_handle = service_control_handler::register(SERVICE_NAME, event_handler)?;
    let next_status = ServiceStatus {
        service_type: ServiceType::OWN_PROCESS,
        current_state: ServiceState::Running,
        controls_accepted: ServiceControlAccept::STOP,
        exit_code: ServiceExitCode::Win32(0),
        checkpoint: 0,
        wait_hint: Duration::default(),
        process_id: None,
    };
    status_handle.set_service_status(next_status)?;

    let mut rt = Runtime::new().unwrap();
    rt.block_on(async move {
        let port = 5902_u16;
        let url = format!("0.0.0.0:{}", port);
        let http_addr: SocketAddr = format!("0.0.0.0:{}", port + 1).parse().unwrap();
        app.server.enable_broadcast_reqrep(true);
        app.server.listen_ws(url).await;
        app.server.listen_http(http_addr).await;
        app.run(rx).await;
    });

    Ok(())
}

fn main() -> Result<()> {
    service_dispatcher::start(SERVICE_NAME, ffi_service_main)?;
    Ok(())
}