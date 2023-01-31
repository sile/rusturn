rusturn
=======

[![rusturn](https://img.shields.io/crates/v/rusturn.svg)](https://crates.io/crates/rusturn)
[![Documentation](https://docs.rs/rusturn/badge.svg)](https://docs.rs/rusturn)
[![Actions Status](https://github.com/sile/rusturn/workflows/CI/badge.svg)](https://github.com/sile/rusturn/actions)
[![Coverage Status](https://coveralls.io/repos/github/sile/rusturn/badge.svg?branch=master)](https://coveralls.io/github/sile/rusturn?branch=master)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)

A Rust implementation of [TURN][RFC 5766] server and client.

[Documentation](https://docs.rs/rusturn)


Examples
--------

```rust
use futures::Future;
use rustun::message::Request;
use rustun::transport::StunUdpTransporter;
use rusturn::auth::AuthParams;
use rusturn::transport::UdpOverTurnTransporter;
use stun_codec::{rfc5389, MessageDecoder, MessageEncoder};

let client_auth_params = AuthParams::new("foo", "bar")?;
let server_auth_params =
    AuthParams::with_realm_and_nonce("foo", "bar", "baz", "qux")?;

// STUN server (peer)
let stun_server = fibers_global::execute(rustun::server::UdpServer::start(
    fibers_global::handle(),
    "127.0.0.1:0".parse().unwrap(),
    rustun::server::BindingHandler,
))?;
let stun_server_addr = stun_server.local_addr();
fibers_global::spawn(stun_server.map(|_| ()).map_err(|e| panic!("{}", e)));

// TURN server
let turn_server = fibers_global::execute(rusturn::server::UdpServer::start(
    "127.0.0.1:0".parse().unwrap(),
    server_auth_params,
))?;
let turn_server_addr = turn_server.local_addr();
fibers_global::spawn(turn_server.map_err(|e| panic!("{}", e)));

// TURN client
let turn_client = fibers_global::execute(rusturn::client::UdpClient::allocate(
    turn_server_addr,
    client_auth_params
))?;
let transporter =
    UdpOverTurnTransporter::<_, MessageEncoder<_>, MessageDecoder<_>>::new(turn_client);

// STUN client (over TURN)
let stun_channel = rustun::channel::Channel::new(StunUdpTransporter::new(transporter));
let stun_client = rustun::client::Client::new(&fibers_global::handle(), stun_channel);

// BINDING request
let request = Request::<rfc5389::Attribute>::new(rfc5389::methods::BINDING);
let response = fibers_global::execute(
    stun_client.call(stun_server_addr, request)
)?;
assert!(response.is_ok(), "{:?}", response);
```

References
----------

- [RFC 5766: Traversal Using Relays around NAT (TURN)][RFC 5766]

[RFC 5766]: https://tools.ietf.org/html/rfc5766
