use crate::*;

pub async fn http_handler(req: Request<Incoming>, dns_cache: DnsCache, _args: &'static Args) -> Result<Response<EitherBody<FullBody, hyper_staticfile::Body>>, BoxedError> {
    // Check path
    let path = req.uri().path();
    if !path.starts_with("/mantalon-connect/") && path != "/mantalon-connect" {
        let mut response = Response::new(FullBody::from("Endpoint not found. Try /mantalon-connect or see the GitHub at https://github.com/Mubelotix/mantalon"));
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
    let ips = match protocols.next() {
        Some(Protocol::Ip4(ip)) => vec![IpAddr::V4(ip)],
        Some(Protocol::Ip6(ip)) => vec![IpAddr::V6(ip)],
        Some(Protocol::Dns(domain) | Protocol::Dnsaddr(domain)) => resolve(dns_cache, &domain, SocketAddr::new(IpAddr::V4(Ipv4Addr::from([8,8,8,8])), 53)).await,
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
            'try_ip: {
                for ip in &ips {
                    let addr = SocketAddr::new(*ip, port);
                    let stream = match TcpStream::connect(addr).await {
                        Ok(stream) => stream,
                        Err(e) => {
                            error!("Could not connect to address: {e}");
                            continue;
                        },
                    };
                    let (transport_reader, transport_write) = stream.into_split();
                    break 'try_ip (Box::new(transport_reader), Box::new(transport_write))
                }
                let mut response = Response::new(FullBody::from(format!("Could not connect to any ip: {ips:?}")));
                *response.status_mut() = StatusCode::INTERNAL_SERVER_ERROR;
                return Ok(response.map(EitherBody::Left));
            }
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
    debug!("Transport established to {addr}");

    // Ensure there are no more protocols
    if let Some(p) = protocols.next() {
        debug!("Unsupported protocol: {p}");
        let mut response = Response::new(FullBody::from(format!("Unsupported protocol: {p}")));
        *response.status_mut() = StatusCode::INTERNAL_SERVER_ERROR;
        return Ok(response.map(EitherBody::Left));
    }

    // Create handshake server
    let mut server = Server::new();
    
    // Compression is useless as the data is encrypted
    // #[cfg(feature = "deflate")]
    // {
    //     let deflate = soketto::extension::deflate::Deflate::new(soketto::Mode::Server);
    //     server.add_extension(Box::new(deflate));
    // }

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

pub type WsSender = Sender<BufReader<BufWriter<Compat<TokioIo<Upgraded>>>>>;
pub type WsReceiver = Receiver<BufReader<BufWriter<Compat<TokioIo<Upgraded>>>>>;

pub async fn handshake(server: Server, req: Request<Incoming>) -> Result<(WsSender, WsReceiver), BoxedError> {
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
