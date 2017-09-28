extern crate clap;
extern crate fibers;
extern crate rustun;
extern crate rusturn;
#[macro_use]
extern crate trackable;

use std::net::SocketAddr;
use clap::{App, Arg};
use fibers::{Executor, InPlaceExecutor, Spawn};
use rustun::{Client, Method, Error};
use rustun::message::{Request, Response, Message};
use rustun::client::UdpClient;
use rustun::rfc5389::attributes::{Realm, Username, Nonce, MessageIntegrity};
use rusturn::Attribute;
use rusturn::rfc5766::methods::{Allocate, CreatePermission};
use rusturn::rfc5766::attributes::{RequestedTransport, XorPeerAddress};

fn main() {
    let matches = App::new("turncli")
        .arg(Arg::with_name("TCP").long("tcp"))
        .arg(
            Arg::with_name("SERVER")
                .short("s")
                .long("server")
                .takes_value(true)
                .default_value("127.0.0.1:3478"),
        )
        .arg(
            Arg::with_name("USERNAME")
                .short("u")
                .long("username")
                .takes_value(true)
                .default_value("foo"),
        )
        .arg(
            Arg::with_name("PASSWORD")
                .long("password")
                .takes_value(true)
                .default_value("password"),
        )
        .get_matches();

    let server = matches.value_of("SERVER").unwrap();
    let server: SocketAddr = server.parse().expect("Invalid UDP address");

    let username = matches.value_of("USERNAME").unwrap();
    let password = matches.value_of("PASSWORD").unwrap();

    let mut executor = InPlaceExecutor::new().unwrap();

    let mut client = UdpClient::new(&executor.handle(), server);

    // Allocation
    let response = track_try_unwrap!(call(&mut executor, &mut client, Allocate.request()));
    println!("[1] ALLOCATE response: {:?}", response);
    assert!(response.is_err());

    let username = Username::new(username.to_string()).unwrap();
    let realm = response.get_attribute::<Realm>().unwrap().clone();
    let nonce = response.get_attribute::<Nonce>().unwrap().clone();
    let mut request = Allocate.request();
    request.add_attribute(username.clone());
    request.add_attribute(realm.clone());
    request.add_attribute(nonce.clone());
    request.add_attribute(RequestedTransport::new());
    let mi = MessageIntegrity::new_long_term_credential(&request, &username, &realm, password)
        .unwrap();
    request.add_attribute(mi);

    let response = track_try_unwrap!(call(&mut executor, &mut client, request.clone()));
    println!("[2] ALLOCATE response: {:?}", response);
    assert!(response.is_ok());
    let mi = response.get_attribute::<MessageIntegrity>().unwrap();
    track_try_unwrap!(mi.check_long_term_credential(&username, &realm, password));

    // Permission
    let peer = XorPeerAddress::new("127.0.0.1:4000".parse().unwrap());
    let mut request = CreatePermission.request();
    request.add_attribute(username.clone());
    request.add_attribute(realm.clone());
    request.add_attribute(nonce);
    request.add_attribute(peer);
    let mi = MessageIntegrity::new_long_term_credential(&request, &username, &realm, password)
        .unwrap();
    request.add_attribute(mi);

    let response = track_try_unwrap!(call(&mut executor, &mut client, request.clone()));
    println!("[3] CREATE-PERMISSION response: {:?}", response);
    assert!(response.is_ok());
    let mi = response.get_attribute::<MessageIntegrity>().unwrap();
    track_try_unwrap!(mi.check_long_term_credential(&username, &realm, password));
}

fn call<E: Executor, M: Method + Send + 'static>(
    executor: &mut E,
    client: &mut UdpClient,
    request: Request<M, Attribute>,
) -> Result<Response<M, Attribute>, Error> {
    let monitor = executor.handle().spawn_monitor(client.call(request));
    let response = track_try!(executor.run_fiber(monitor).unwrap());
    Ok(response)
}
