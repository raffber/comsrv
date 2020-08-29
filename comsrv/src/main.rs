use clap::{App as ClapApp, Arg};
use std::process::exit;
use tokio::runtime::Runtime;
use comsrv::app::App;

fn main() {
    env_logger::init();
    let matches = ClapApp::new("Async communication server")
        .version("0.1")
        .author("Raphael Bernhard <beraphae@gmail.com>")
        .about("Multiplex communication to instruments over RPC")
        .arg(Arg::with_name("port")
            .long("port")
            .short("p")
            .default_value("6428")
            .help("Define the port to listen on."))
        .get_matches();

    let port = matches.value_of("port").unwrap().to_string();
    let port = match port.parse::<u16>() {
        Ok(port) => port,
        Err(_) => {
            println!("Cannot parse `{}` as a port number.", port);
            exit(1);
        },
    };


    let mut rt = Runtime::new().unwrap();
    rt.block_on(async move {
        let app = App::new();
        app.run(port).await;
    });
}
