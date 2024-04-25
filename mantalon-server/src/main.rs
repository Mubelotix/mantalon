use std::net::{IpAddr, SocketAddr};

use futures::io::{BufReader, BufWriter};
use hyper::body::Incoming;
use hyper::server::conn::http1::Builder as HttpBuilder;
use hyper::upgrade::Upgraded;
use hyper::{Method, StatusCode};
use hyper::{body::Bytes, service::service_fn, Request, Response};
use hyper_util::rt::TokioIo;
use log::{debug, error, info, trace};
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

    let addr: SocketAddr = ([127, 0, 0, 1], 8080).into();
    let listener = TcpListener::bind(addr).await?;

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

        tokio::spawn(async {
            let io = TokioIo::new(stream);
            let conn = HttpBuilder::new().serve_connection(io, service_fn(http_handler));            
            let conn = conn.with_upgrades(); // Enable upgrades on the connection for the websocket upgrades to work.
            if let Err(err) = conn.await {
                error!("HTTP connection failed {err}");
            }
        });
    }
}

async fn http_handler(req: Request<Incoming>) -> Result<Response<FullBody>, BoxedError> {
    // Check path
    let path = req.uri().path();
    if !path.starts_with("/connect/") && path != "/connect" {
        debug!("Endpoint not found: {path}");
        let mut response = Response::new(FullBody::from("Endpoint not found. Try /connect"));
        *response.status_mut() = StatusCode::NOT_FOUND;
        return Ok(response);
    }

    // Check method
    if req.method() != Method::GET && req.method() != Method::POST {
        debug!("Method not allowed: {}", req.method());
        let mut response = Response::new(FullBody::from("Method not allowed. Try GET or POST"));
        *response.status_mut() = StatusCode::METHOD_NOT_ALLOWED;
        return Ok(response);
    }

    // Check if it's a websocket upgrade request
    if !is_upgrade_request(&req) {
        debug!("Upgrade to websocket required");
        let mut response = Response::new(FullBody::from("Upgrade to websocket required"));
        *response.status_mut() = StatusCode::UPGRADE_REQUIRED;
        return Ok(response);
    }

    // Extract the address from the path
    let addr = &path[8..];
    let addr: Multiaddr = match addr.parse() {
        Ok(addr) => addr,
        Err(e) => {
            debug!("Invalid address: {e}");
            let mut response = Response::new(FullBody::from(format!("Invalid address: {e}")));
            *response.status_mut() = StatusCode::BAD_REQUEST;
            return Ok(response);
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
            return Ok(response);
        }
        None => {
            debug!("Incomplete address");
            let mut response = Response::new(FullBody::from("Incomplete address. Try something like /ip4/127.0.0.1/tcp/8080"));
            *response.status_mut() = StatusCode::BAD_REQUEST;
            return Ok(response);
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
                    return Ok(response);
                }
            };
            let (transport_reader, transport_write) = stream.into_split();
            (Box::new(transport_reader), Box::new(transport_write))
        }
        Some(p) => {
            debug!("Unsupported protocol: {p}");
            let mut response = Response::new(FullBody::from(format!("Unsupported protocol: {p}")));
            *response.status_mut() = StatusCode::INTERNAL_SERVER_ERROR;
            return Ok(response);
        }
        None => {
            debug!("Incomplete address");
            let mut response = Response::new(FullBody::from("Incomplete address. Try something like /ip4/127.0.0.1/tcp/8080"));
            *response.status_mut() = StatusCode::BAD_REQUEST;
            return Ok(response);
        }
    };

    // Ensure there are no more protocols
    if let Some(p) = protocols.next() {
        debug!("Unsupported protocol: {p}");
        let mut response = Response::new(FullBody::from(format!("Unsupported protocol: {p}")));
        *response.status_mut() = StatusCode::INTERNAL_SERVER_ERROR;
        return Ok(response);
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
            return Ok(response);
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
    Ok(response.map(|()| FullBody::default()))
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
                trace!("Sending {}", String::from_utf8_lossy(&message));
                writer.write_all(&message).await.unwrap();
                writer.flush().await.unwrap();
            }
            Ok(Data::Text(n)) => {
                assert_eq!(n, message.len());
                trace!("Sending {}", String::from_utf8_lossy(&message));
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

async fn relay_transport_to_websocket(mut reader: Box<dyn AsyncRead + Send + Unpin>, mut sender: WsSender) {
    let mut buffer = [0; 1024];
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
        trace!("Received {}", String::from_utf8_lossy(&buffer[..n]));
        sender.send_binary(&buffer[..n]).await.unwrap();
        sender.flush().await.unwrap();
    }
}
