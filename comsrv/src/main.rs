use std::net::SocketAddr;
use std::process::exit;

use clap::{App as ClapApp, Arg};
use tokio::runtime::Runtime;

use comsrv::app::App;
use env_logger::Env;

fn main() {
    let matches = ClapApp::new("Async communication server")
        .version("0.1")
        .author("Raphael Bernhard <beraphae@gmail.com>")
        .about("Multiplex communication to instruments over RPC")
        .arg(
            Arg::with_name("port")
                .long("port")
                .short("p")
                .default_value("5902")
                .help("Define the port to listen on."),
        )
        .arg(
            Arg::with_name("verbose")
                .long("verbose")
                .short("v")
                .help("Log verbose output"),
        )
        .get_matches();

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

    let rt = Runtime::new().unwrap();
    rt.block_on(async move {
        let (app, rx) = App::new();

        let url = format!("0.0.0.0:{}", port);
        let http_addr: SocketAddr = format!("0.0.0.0:{}", port + 1).parse().unwrap();
        app.server.enable_broadcast_reqrep(true);
        println!("Listening on ws://{}", url);
        app.server.listen_ws(url).await;
        println!("Listening on http://{}", http_addr);
        app.server.listen_http(http_addr).await;
        app.run(rx).await;
        log::debug!("Application quitting.");
    });
}
