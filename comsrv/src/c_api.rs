#![allow(unsafe_code)]

use std::{net::SocketAddr, time::Duration};

use tokio::runtime::Runtime;

use crate::app::App;

#[repr(C)]
pub struct AppState {
    app: *const App,
}

#[no_mangle]
pub extern "C" fn comsrv_spawn() -> AppState {
    let port = 5902;
    let (tx, rx) = std::sync::mpsc::channel();

    let rt = Runtime::new().unwrap();
    rt.block_on(async move {
        let (app, rx) = App::new();
        let app = Box::new(app);
        let ws_addr: SocketAddr = format!("127.0.0.1:{}", port).parse().unwrap();
        let http_addr: SocketAddr = format!("127.0.0.1:{}", port + 1).parse().unwrap();
        println!("Listening on ws://{}", ws_addr);
        println!("Listening on http://{}", http_addr);
        app.server.listen_ws(&ws_addr).await.unwrap();
        app.server.listen_http(&http_addr).await;

        let ret = AppState {
            app: app.as_ref() as *const App,
        };

        tx.send(ret).unwrap();

        app.run(rx).await;
        std::mem::forget(app);
        log::debug!("Application quitting.");
    });

    return rx.recv_timeout(Duration::from_secs(1)).expect("Failed to start comsrv");
}

#[no_mangle]
pub extern "C" fn comsrv_stop(state: AppState) {
    let app: &App = unsafe { &*state.app };
    app.server.shutdown();
    log::debug!("Application stopping.");
}

#[no_mangle]
pub extern "C" fn comsrv_destroy(state: AppState) {
    let app: Box<App> = unsafe { Box::from_raw(state.app as *mut App) };
    drop(app);
}
