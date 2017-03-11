extern crate clap;
#[macro_use]
extern crate slog;
extern crate slog_term;
extern crate fibers;
extern crate rustun;
extern crate rusturn;

use clap::{App, Arg};
use slog::{Logger, DrainExt, Record, LevelFilter};
use fibers::{Executor, InPlaceExecutor, Spawn};
use rustun::server::UdpServer;
use rusturn::server::DefaultHandler;

fn main() {
    let matches = App::new("turnsrv")
        .arg(Arg::with_name("PORT")
                 .short("p")
                 .long("port")
                 .takes_value(true)
                 .default_value("3478"))
        .arg(Arg::with_name("PASSWORD").long("password").takes_value(true).default_value("password"))
        .get_matches();

    let port = matches.value_of("PORT").unwrap();
    let addr = format!("0.0.0.0:{}", port).parse().expect("Invalid UDP address");

    let place_fn = |info: &Record| format!("{}:{}", info.module(), info.line());
    let logger = Logger::root(LevelFilter::new(slog_term::streamer().build(), slog::Level::Debug)
                                  .fuse(),
                              o!("place" => place_fn));

    let password = matches.value_of("PASSWORD").unwrap();
    let mut handler = DefaultHandler::with_logger(logger);
    handler.set_password(password);

    let mut executor = InPlaceExecutor::new().unwrap();
    let spawner = executor.handle();
    let monitor = executor.spawn_monitor(UdpServer::new(addr).start(spawner.boxed(), handler));
    let result = executor.run_fiber(monitor).unwrap();
    println!("RESULT: {:?}", result);
}
