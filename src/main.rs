
use std::{collections::HashMap, env, hash::{DefaultHasher, Hash, Hasher}, net::IpAddr, net::SocketAddr, str::FromStr, time::Duration};
use actix_web::{get, http::StatusCode, post, rt::{net::{TcpSocket, TcpStream}, spawn}, web::{Data, Json}, App, HttpRequest, HttpResponse, HttpServer, Responder, ResponseError};
use multiaddr::{Multiaddr, Protocol};
use actix::{Actor, StreamHandler};
use actix_web::{web, Error};
use actix_web_actors::ws::{self, WebsocketContext, Message as WsMessage, ProtocolError as WsProtocolError};

mod errors;
use errors::*;


/// Define HTTP actor
struct WebsocketTcpProxy {
    stream: TcpStream,
}

impl Actor for WebsocketTcpProxy {
    type Context = WebsocketContext<Self>;
}

/// Handler for ws::Message message
impl StreamHandler<Result<WsMessage, WsProtocolError>> for WebsocketTcpProxy {
    fn handle(&mut self, msg: Result<WsMessage, WsProtocolError>, ctx: &mut Self::Context) {
        match msg {
            Ok(WsMessage::Ping(msg)) => ctx.pong(&msg),
            Ok(WsMessage::Text(text)) => ctx.text(text),
            Ok(WsMessage::Binary(bin)) => ctx.binary(bin),
            _ => (),
        }
    }
}

#[post("/connect/{addr:.*}")]
async fn connect(req: HttpRequest, payload: web::Payload) -> Result<HttpResponse, MantalonError> {
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

    match protocols.next() {
        Some(Protocol::Tcp(port)) => {
            let socket = match ip {
                IpAddr::V4(_) => TcpSocket::new_v4().unwrap(),
                IpAddr::V6(_) => TcpSocket::new_v6().unwrap(),
            };
            let socket_addr = SocketAddr::new(ip, port);
            let stream = socket.connect(socket_addr).await.map_err(|error| MantalonError::ConnectionError { error })?;
            let proxy = WebsocketTcpProxy { stream };
            ws::start(proxy, &req, payload)
        },
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
