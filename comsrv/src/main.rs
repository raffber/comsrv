use std::net::SocketAddr;
use std::process::exit;

use clap::{crate_authors, crate_version, App as ClapApp, Arg};
use tokio::runtime::Runtime;

use comsrv::app::App;
use env_logger::Env;

fn main() {
    let matches = ClapApp::new("Async communication server")
        .author(crate_authors!())
        .version(crate_version!())
        .about("Multiplex communication to instruments over RPC")
        .arg(
            Arg::with_name("port")
                .long("port")
                .short('p')
                .default_value("5902")
                .help("Define the port to listen on."),
        )
        .arg(
            Arg::with_name("http-port")
                .long("http-port")
                .help("Define the port to listen on for HTTP."),
        )
        .arg(
            Arg::with_name("broadcast_reqrep")
                .long("broadcast-requests")
                .short('b')
                .help("Broadcast requests back to RPC bus"),
        )
        .arg(Arg::with_name("verbose").long("verbose").short('v').help("Log verbose output"))
        .get_matches();

    let broadcast_reqrep = matches.is_present("broadcast_reqrep");
    let verbose = matches.is_present("verbose");
    if verbose {
        env_logger::Builder::from_env(Env::default().default_filter_or("comsrv=debug")).init();
    } else {
        env_logger::init();
    }

    let port = matches.value_of("port").unwrap().to_string();
    let port = match port.parse::<u16>() {
        Ok(port) => port,
        Err(_) => {
            println!("Cannot parse `{}` as a port number.", port);
            exit(1);
        }
    };

    let http_port = matches.value_of("http-port").map(|http_port| match http_port.parse::<u16>() {
        Ok(port) => port,
        Err(_) => {
            println!("Cannot parse `{}` as a port number.", http_port);
            exit(1);
        }
    });

    let rt = Runtime::new().unwrap();
    rt.block_on(async move {
        let (app, rx) = App::new();
        app.server.enable_broadcast_reqrep(broadcast_reqrep);

        let ws_addr: SocketAddr = format!("0.0.0.0:{}", port).parse().unwrap();
        println!("Listening on ws://{}", ws_addr);

        app.server.listen_ws(&ws_addr).await.expect("Failed to listen on WebSocket");

        if let Some(http_port) = http_port {
            let http_addr: SocketAddr = format!("0.0.0.0:{}", http_port).parse().unwrap();
            println!("Listening on http://{}", http_addr);
            app.server.listen_http(&http_addr).await;
        }

        app.run(rx).await;
        log::debug!("Application quitting.");
    });
}
