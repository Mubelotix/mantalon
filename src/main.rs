use std::net::SocketAddr;

use futures::io::{BufReader, BufWriter};
use hyper::body::Incoming;
use hyper::server::conn::http1::Builder as HttpBuilder;
use hyper::{body::Bytes, service::service_fn, Request, Response};
use hyper_util::rt::TokioIo;
use log::{error, info};
use soketto::Data;
use soketto::{
    handshake::http::{is_upgrade_request, Server},
    BoxedError,
};
use tokio::net::TcpListener;
use tokio_util::compat::TokioAsyncReadCompatExt;
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
            let conn = HttpBuilder::new().serve_connection(io, service_fn(handler));            
            let conn = conn.with_upgrades(); // Enable upgrades on the connection for the websocket upgrades to work.
            if let Err(err) = conn.await {
                error!("HTTP connection failed {err}");
            }
        });
    }
}

/// Handle incoming HTTP Requests.
async fn handler(req: Request<Incoming>) -> Result<Response<FullBody>, BoxedError> {
    if !is_upgrade_request(&req) {
        return Ok(Response::new(FullBody::from("Hello HTTP!")));
    }

    let mut server = Server::new();

    // Add any extensions that we want to use.
    #[cfg(feature = "deflate")]
    {
        let deflate = soketto::extension::deflate::Deflate::new(soketto::Mode::Server);
        server.add_extension(Box::new(deflate));
    }

    // Attempt the handshake.
    match server.receive_request(&req) {
        // The handshake has been successful so far; return the response we're given back
        // and spawn a task to handle the long-running WebSocket server:
        Ok(response) => {
            tokio::spawn(async move {
                if let Err(e) = websocket_echo_messages(server, req).await {
                    error!("Error upgrading to websocket connection: {e}");
                }
            });
            Ok(response.map(|()| FullBody::default()))
        }
        // We tried to upgrade and failed early on; tell the client about the failure however we like:
        Err(e) => {
            error!("Could not upgrade connection: {e}");
            Ok(Response::new(FullBody::from("Something went wrong upgrading!")))
        }
    }
}

/// Echo any messages we get from the client back to them
async fn websocket_echo_messages(server: Server, req: Request<Incoming>) -> Result<(), BoxedError> {
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
    let (mut sender, mut receiver) = server.into_builder(stream).finish();

    // Echo any received messages back to the client:
    let mut message = Vec::new();
    loop {
        message.clear();
        match receiver.receive_data(&mut message).await {
            Ok(Data::Binary(n)) => {
                assert_eq!(n, message.len());
                sender.send_binary_mut(&mut message).await?;
                sender.flush().await?
            }
            Ok(Data::Text(n)) => {
                assert_eq!(n, message.len());
                if let Ok(txt) = std::str::from_utf8(&message) {
                    sender.send_text(txt).await?;
                    sender.flush().await?
                } else {
                    break;
                }
            }
            Err(SockettoError::Closed) => break,
            Err(e) => {
                error!("Websocket connection error: {e}");
                break;
            }
        }
    }

    Ok(())
}
