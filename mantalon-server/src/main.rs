use std::net::{IpAddr, SocketAddr};
use futures::io::{BufReader, BufWriter};
use http_body_util::Either as EitherBody;
use hyper::body::Incoming;
use hyper::server::conn::http1::Builder as HttpBuilder;
use hyper::upgrade::Upgraded;
use hyper::{Method, StatusCode, Uri};
use hyper::{body::Bytes, service::service_fn, Request, Response};
use hyper_staticfile::Static;
use hyper_util::rt::TokioIo;
use log::{debug, error, info, trace, warn};
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

type FullBody = http_body_util::Full<Bytes>;

/// Start up a hyper server.
#[tokio::main]
async fn main() -> Result<(), BoxedError> {
    env_logger::init();

    let addr: SocketAddr = ([127, 0, 0, 1], 8000).into();
    let listener = TcpListener::bind(addr).await?;

    info!("Listening on http://{:?}", listener.local_addr().unwrap());

    let static_files = Static::new("mantalon-client");

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
        tokio::spawn(async move {
            let io = TokioIo::new(stream);
            let conn = HttpBuilder::new().serve_connection(io, service_fn(move |r| http_handler(r, static_files.clone())));            
            let conn = conn.with_upgrades(); // Enable upgrades on the connection for the websocket upgrades to work.
            if let Err(err) = conn.await {
                error!("HTTP connection failed {err}");
            }
        });
    }
}

async fn http_handler(mut req: Request<Incoming>, static_files: Static) -> Result<Response<EitherBody<FullBody, hyper_staticfile::Body>>, BoxedError> {
    // Check path
    let path = req.uri().path();
    debug!("path {path}");
    if path.starts_with("/pkg/") || path == "/sw.js" {
        debug!("Serving static file: {}", req.uri());
        return match static_files.serve(req).await {
            Ok(response) => Ok(response.map(EitherBody::Right)),
            Err(e) => {
                error!("Static file error: {e}");
                let mut response = Response::new(FullBody::from("Internal server error"));
                *response.status_mut() = StatusCode::INTERNAL_SERVER_ERROR;
                Ok(response.map(EitherBody::Left))
            }
        };
    }
    if !path.starts_with("/mantalon-connect/") && path != "/mantalon-connect" {
        if req.method() == Method::GET {
            let mut uri = req.uri().clone().into_parts();
            uri.path_and_query = Some("index.html".parse().unwrap());
            *req.uri_mut() = Uri::from_parts(uri).unwrap();

            return match static_files.serve(req).await {
                Ok(response) => Ok(response.map(EitherBody::Right)),
                Err(e) => {
                    error!("Static file error: {e}");
                    let mut response = Response::new(FullBody::from("Internal server error"));
                    *response.status_mut() = StatusCode::INTERNAL_SERVER_ERROR;
                    Ok(response.map(EitherBody::Left))
                }
            };
        }

        warn!("Endpoint not found: {path}");
        let mut response = Response::new(FullBody::from("Endpoint not found. Try /mantalon-connect"));
        *response.status_mut() = StatusCode::NOT_FOUND;
        return Ok(response.map(EitherBody::Left));
    }

    // Check method
    if req.method() != Method::GET && req.method() != Method::POST {
        debug!("Method not allowed: {}", req.method());
        let mut response = Response::new(FullBody::from("Method not allowed. Try GET or POST"));
        *response.status_mut() = StatusCode::METHOD_NOT_ALLOWED;
        return Ok(response.map(EitherBody::Left));
    }

    // Check if it's a websocket upgrade request
    if !is_upgrade_request(&req) {
        debug!("Upgrade to websocket required");
        let mut response = Response::new(FullBody::from("Upgrade to websocket required"));
        *response.status_mut() = StatusCode::UPGRADE_REQUIRED;
        return Ok(response.map(EitherBody::Left));
    }

    // Extract the address from the path
    let addr = &path[17..];
    let addr: Multiaddr = match addr.parse() {
        Ok(addr) => addr,
        Err(e) => {
            debug!("Invalid address: {e}");
            let mut response = Response::new(FullBody::from(format!("Invalid address: {e}")));
            *response.status_mut() = StatusCode::BAD_REQUEST;
            return Ok(response.map(EitherBody::Left));
        }
    };

    // Extract the IP address from the multiaddr
    let mut protocols = addr.iter();
    let ip = match protocols.next() {
        Some(Protocol::Ip4(ip)) => IpAddr::V4(ip),
        Some(Protocol::Ip6(ip)) => IpAddr::V6(ip),
        Some(p) => {
            debug!("Unsupported protocol: {p}");
            let mut response = Response::new(FullBody::from(format!("Unsupported protocol: {p}")));
            *response.status_mut() = StatusCode::INTERNAL_SERVER_ERROR;
            return Ok(response.map(EitherBody::Left));
        }
        None => {
            debug!("Incomplete address");
            let mut response = Response::new(FullBody::from("Incomplete address. Try something like /ip4/127.0.0.1/tcp/8080"));
            *response.status_mut() = StatusCode::BAD_REQUEST;
            return Ok(response.map(EitherBody::Left));
        }
    };

    // Build the underlying transport
    let (transport_reader, transport_write): (Box<dyn AsyncRead + Send + Unpin>, Box<dyn AsyncWrite + Send + Unpin>) = match protocols.next() {
        Some(Protocol::Tcp(port)) => {
            let addr = SocketAddr::new(ip, port);
            let stream = match TcpStream::connect(addr).await {
                Ok(stream) => stream,
                Err(e) => {
                    error!("Could not connect to address: {e}");
                    let mut response = Response::new(FullBody::from(format!("Could not connect to address: {e}")));
                    *response.status_mut() = StatusCode::INTERNAL_SERVER_ERROR;
                    return Ok(response.map(EitherBody::Left));
                }
            };
            let (transport_reader, transport_write) = stream.into_split();
            (Box::new(transport_reader), Box::new(transport_write))
        }
        Some(p) => {
            debug!("Unsupported protocol: {p}");
            let mut response = Response::new(FullBody::from(format!("Unsupported protocol: {p}")));
            *response.status_mut() = StatusCode::INTERNAL_SERVER_ERROR;
            return Ok(response.map(EitherBody::Left));
        }
        None => {
            debug!("Incomplete address");
            let mut response = Response::new(FullBody::from("Incomplete address. Try something like /ip4/127.0.0.1/tcp/8080"));
            *response.status_mut() = StatusCode::BAD_REQUEST;
            return Ok(response.map(EitherBody::Left));
        }
    };

    // Ensure there are no more protocols
    if let Some(p) = protocols.next() {
        debug!("Unsupported protocol: {p}");
        let mut response = Response::new(FullBody::from(format!("Unsupported protocol: {p}")));
        *response.status_mut() = StatusCode::INTERNAL_SERVER_ERROR;
        return Ok(response.map(EitherBody::Left));
    }

    // Create handshake server
    let mut server = Server::new();
    #[cfg(feature = "deflate")]
    {
        let deflate = soketto::extension::deflate::Deflate::new(soketto::Mode::Server);
        server.add_extension(Box::new(deflate));
    }

    // Attempt the upgrade.
    let response = match server.receive_request(&req) {
        Ok(response) => response,
        Err(e) => {
            error!("Could not upgrade connection: {e}");
            let mut response = Response::new(FullBody::from(format!("Could not upgrade connection: {e}")));
            *response.status_mut() = StatusCode::INTERNAL_SERVER_ERROR;
            return Ok(response.map(EitherBody::Left));
        }
    };

    // Return the response we're given back and spawn a task to handle the long-running WebSocket server
    tokio::spawn(async move {
        let (sender, receiver) = match handshake(server, req).await {
            Ok((sender, receiver)) => (sender, receiver),
            Err(e) => {
                error!("Could not complete handshake: {e}");
                return;
            }
        };
        let fut1 = relay_websocket_to_transport(receiver, transport_write);
        let fut2 = relay_transport_to_websocket(transport_reader, sender);
        
        debug!("Relay now operational");
        tokio::select! {
            _ = fut1 => debug!("Websocket to transport task finished"),
            _ = fut2 => debug!("Transport to websocket task finished"),
        }
    });
    Ok(response.map(|()| FullBody::default()).map(EitherBody::Left))
}

type WsSender = Sender<BufReader<BufWriter<Compat<TokioIo<Upgraded>>>>>;
type WsReceiver = Receiver<BufReader<BufWriter<Compat<TokioIo<Upgraded>>>>>;

async fn handshake(server: Server, req: Request<Incoming>) -> Result<(WsSender, WsReceiver), BoxedError> {
    // The negotiation to upgrade to a WebSocket connection has been successful so far. Next, we get back the underlying
    // stream using `hyper::upgrade::on`, and hand this to a Soketto server to use to handle the WebSocket communication
    // on this socket.
    //
    // Note: awaiting this won't succeed until the handshake response has been returned to the client, so this must be
    // spawned on a separate task so as not to block that response being handed back.
    let stream = hyper::upgrade::on(req).await?;
    let io = TokioIo::new(stream);
    let stream = BufReader::new(BufWriter::new(io.compat()));

    // Get back a reader and writer that we can use to send and receive websocket messages.
    Ok(server.into_builder(stream).finish())
}

async fn relay_websocket_to_transport(mut receiver: WsReceiver, mut writer: Box<dyn AsyncWrite + Send + Unpin>) {
    let mut message = Vec::new();
    loop {
        message.clear();
        match receiver.receive_data(&mut message).await {
            Ok(Data::Binary(n)) => {
                assert_eq!(n, message.len());
                writer.write_all(&message).await.unwrap();
                writer.flush().await.unwrap();
            }
            Ok(Data::Text(n)) => {
                assert_eq!(n, message.len());
                writer.write_all(&message).await.unwrap();
                writer.flush().await.unwrap();
            }
            Err(SockettoError::Closed) => break,
            Err(e) => {
                error!("Websocket connection error: {e}");
                break;
            }
        }
    }
}

#[allow(clippy::uninit_vec)]
async fn relay_transport_to_websocket(mut reader: Box<dyn AsyncRead + Send + Unpin>, mut sender: WsSender) {
    let mut buffer = Vec::with_capacity(100_000);
    unsafe {
        buffer.set_len(buffer.capacity());
    }
    loop {
        let n = match reader.read(&mut buffer).await {
            Ok(n) => n,
            Err(e) => {
                error!("Transport read error: {e}");
                break;
            }
        };
        if n == 0 {
            break;
        }
        sender.send_binary(&buffer[..n]).await.unwrap();
        sender.flush().await.unwrap();
    }
}
