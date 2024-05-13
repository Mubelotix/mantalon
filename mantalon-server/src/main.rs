use clap::Parser;
use futures::io::{BufReader, BufWriter};
use http_body_util::Either as EitherBody;
use hyper::{
    body::{Bytes, Incoming},
    server::conn::http1::Builder as HttpBuilder,
    service::service_fn,
    upgrade::Upgraded,
    Method, Request, Response, StatusCode, Uri,
};
use hyper_staticfile::Static;
use hyper_util::rt::TokioIo;
use log::*;
use multiaddr::{Multiaddr, Protocol};
use soketto::connection::Error as SockettoError;
use soketto::{
    handshake::http::{is_upgrade_request, Server},
    BoxedError, Data, Receiver, Sender,
};
use std::{
    net::{IpAddr, Ipv4Addr, SocketAddr},
    sync::Arc,
};
use tokio::{
    io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt},
    net::{TcpListener, TcpStream},
};
use tokio_util::compat::{Compat, TokioAsyncReadCompatExt};

mod dns;
mod handler;
mod relay;
use {dns::*, handler::*, relay::*};

type FullBody = http_body_util::Full<Bytes>;

/// A proxy server to relay TCP traffic over WebSockets.
#[derive(Parser, Debug)]
#[command(version, about)]
struct Args {
    /// Path to the Mantalon config folder.
    /// Should contain manifest.json and files to inject.
    /// Will be served at /pkg/config
    #[arg(short, long)]
    config_path: Option<String>,

    /// The manifest version.
    /// Updating it will cause all clients to update the service worker and manifest.
    #[arg(short, long, default_value = "default")]
    manifest_version: String,

    /// The library version.
    /// Updating it will cause all clients to update the service worker, manifest and library.
    #[arg(short, long, default_value = "default")]
    lib_version: String,

    /// The port to listen on.
    #[arg(short, long, default_value = "8000")]
    port: u16,
}

/// Start up a hyper server.
#[tokio::main]
async fn main() -> Result<(), BoxedError> {
    let args = Box::new(Args::parse());
    let args: &'static Args = Box::leak(args);
    env_logger::init();

    let addr: SocketAddr = ([127, 0, 0, 1], args.port).into();
    let listener = TcpListener::bind(addr).await?;
    let static_files = Static::new("mantalon-client");
    let config_static_files = Static::new(args.config_path.clone().unwrap_or_default());
    let dns_cache = DnsCache::default();

    info!("Listening on http://{:?}", listener.local_addr().unwrap());

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
        let config_static_files = config_static_files.clone();
        let dns_cache = Arc::clone(&dns_cache);
        let service = service_fn(move |r| http_handler(r, static_files.clone(), config_static_files.clone(), Arc::clone(&dns_cache), args));
        tokio::spawn(async move {
            let io = TokioIo::new(stream);
            let conn = HttpBuilder::new().serve_connection(io, service);
            let conn = conn.with_upgrades(); // Enable upgrades on the connection for the websocket upgrades to work.
            if let Err(err) = conn.await {
                error!("HTTP connection failed {err}");
            }
        });
    }
}

