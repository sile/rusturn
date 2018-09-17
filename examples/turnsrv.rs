// TODO
// extern crate clap;
// #[macro_use]
// extern crate slog;
// extern crate fibers;
// extern crate rustun;
// extern crate rusturn;
// extern crate slog_term;

// use clap::{App, Arg};
// use fibers::{Executor, InPlaceExecutor, Spawn};
// use rustun::server::{TcpServer, UdpServer};
// use rusturn::server::DefaultHandler;
// use slog::{DrainExt, LevelFilter, Logger, Record};
// use std::net::SocketAddr;

fn main() {
    // let matches = App::new("turnsrv")
    //     .arg(Arg::with_name("TCP").long("tcp"))
    //     .arg(
    //         Arg::with_name("ADDR")
    //             .short("a")
    //             .long("addr")
    //             .takes_value(true)
    //             .default_value("127.0.0.1:3478"),
    //     )
    //     .arg(
    //         Arg::with_name("PASSWORD")
    //             .long("password")
    //             .takes_value(true)
    //             .default_value("password"),
    //     )
    //     .get_matches();

    // let addr = matches.value_of("ADDR").unwrap();
    // let addr: SocketAddr = addr.parse().expect("Invalid UDP address");

    // let place_fn = |info: &Record| format!("{}:{}", info.module(), info.line());
    // let logger = Logger::root(
    //     LevelFilter::new(slog_term::streamer().build(), slog::Level::Debug).fuse(),
    //     o!("place" => place_fn),
    // );

    // let password = matches.value_of("PASSWORD").unwrap();

    // let mut executor = InPlaceExecutor::new().unwrap();
    // let mut handler = DefaultHandler::with_logger(logger, executor.handle(), addr.ip());
    // handler.set_password(password);

    // let spawner = executor.handle();
    // let monitor = if matches.is_present("TCP") {
    //     executor.spawn_monitor(TcpServer::new(addr).start(spawner, handler))
    // } else {
    //     let mut server = UdpServer::new(addr);
    //     server.recv_buffer_size(10240);
    //     executor.spawn_monitor(server.start(spawner, handler))
    // };
    // let result = executor.run_fiber(monitor).unwrap();
    // println!("RESULT: {:?}", result);
}
