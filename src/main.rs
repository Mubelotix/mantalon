
use std::{collections::HashMap, env, hash::{DefaultHasher, Hash, Hasher}, net::IpAddr, str::FromStr, time::Duration};
use actix_web::{get, http::StatusCode, post, rt::spawn, web::{Data, Json}, App, HttpRequest, HttpResponse, HttpServer, Responder, ResponseError};
use multiaddr::{Multiaddr, Protocol};

mod errors;
use errors::*;

#[post("/connect/{addr:.*}")]
async fn connect(req: HttpRequest) -> Result<HttpResponse, MantalonError> {
    let mut addr = req.match_info().get("addr").unwrap().to_owned();
    if !addr.starts_with('/') {
        addr.insert(0, '/');
    }
    let addr: Multiaddr = addr.parse().map_err(|error| MantalonError::InvalidAddr { addr, error })?;

    let mut protocols = addr.iter();
    let ip = match protocols.next() {
        Some(Protocol::Ip4(ip)) => IpAddr::V4(ip),
        Some(Protocol::Ip6(ip)) => IpAddr::V6(ip),
        Some(p) => return Err(MantalonError::UnsupportedProtocol { protocol: format!("{p:?}") }),
        None => return Err(MantalonError::MissingProtocol),
    };

    let addr = match protocols.next() {
        Some(Protocol::Tcp(port)) => format!("tcp://{ip}:{}", port),
        Some(Protocol::Udp(port)) => format!("udp://{ip}:{}", port),
        Some(p) => return Err(MantalonError::UnsupportedProtocol { protocol: format!("{p:?}") }),
        None => return Err(MantalonError::MissingProtocol),
    };
    
    Ok(HttpResponse::Ok().body(format!("Connecting to {addr}...")))
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    let server = HttpServer::new(move || {
        App::new().service(connect)
    });

    let port = env::var("PORT").unwrap_or("8080".to_string());
    let addr = format!("127.0.0.1:{port}");
    server.bind(addr).unwrap().run().await
}
