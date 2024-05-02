use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::sync::Arc;
use futures::io::{BufReader, BufWriter};
use http_body_util::Either as EitherBody;
use hyper::body::Incoming;
use hyper::server::conn::http1::Builder as HttpBuilder;
use hyper::upgrade::Upgraded;
use hyper::{Method, StatusCode, Uri};
use hyper::{body::Bytes, service::service_fn, Request, Response};
use hyper_staticfile::Static;
use hyper_util::rt::TokioIo;
use log::*;
use multiaddr::{Multiaddr, Protocol};
use soketto::{Data, Receiver, Sender};
use soketto::{
    handshake::http::{is_upgrade_request, Server},
    BoxedError,
};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio_util::compat::{Compat, TokioAsyncReadCompatExt};
use soketto::connection::Error as SockettoError;

mod dns;
mod handler;
mod relay;
use {dns::*, handler::*, relay::*};

type FullBody = http_body_util::Full<Bytes>;

/// Start up a hyper server.
#[tokio::main]
async fn main() -> Result<(), BoxedError> {
    env_logger::init();

    let addr: SocketAddr = ([127, 0, 0, 1], 8000).into();
    let listener = TcpListener::bind(addr).await?;

    info!("Listening on http://{:?}", listener.local_addr().unwrap());

    let static_files = Static::new("mantalon-client");
    let dns_cache = DnsCache::default();

    loop {
        let stream = match listener.accept().await {
            Ok((stream, addr)) => {
                log::info!("Accepting new connection: {addr}");
                stream
            }
            Err(e) => {
                log::error!("Accepting new connection failed: {e}");
                continue;
            }
        };

        let static_files = static_files.clone();
        let dns_cache = Arc::clone(&dns_cache);
        tokio::spawn(async move {
            let io = TokioIo::new(stream);
            let conn = HttpBuilder::new().serve_connection(io, service_fn(move |r| http_handler(r, static_files.clone(), Arc::clone(&dns_cache))));            
            let conn = conn.with_upgrades(); // Enable upgrades on the connection for the websocket upgrades to work.
            if let Err(err) = conn.await {
                error!("HTTP connection failed {err}");
            }
        });
    }
}

