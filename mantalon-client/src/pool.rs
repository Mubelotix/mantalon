use std::{cell::RefCell, collections::HashMap, io::Error as IoError, rc::Rc};
use http::{Request, Response, Uri};
use tokio::sync::RwLock;
use tokio_rustls::rustls::pki_types::InvalidDnsNameError;
use crate::*;
use lazy_static::lazy_static;

lazy_static!{
    pub static ref POOL: Pool = Pool {
        connections: Default::default(),
    };

    pub static ref MANTALON_ENDPOINT: EndpointUrl = EndpointUrl(Rc::new(RefCell::new(String::new())));
}

#[allow(clippy::type_complexity)]
pub struct Pool {
    connections: Rc<RwLock<HashMap<String, SendRequest>>>,
}

unsafe impl Send for Pool {}
unsafe impl Sync for Pool {}

pub struct EndpointUrl(Rc<RefCell<String>>);
unsafe impl Send for EndpointUrl {}
unsafe impl Sync for EndpointUrl {}
impl EndpointUrl {
    pub fn set(&self, url: String) {
        *self.0.borrow_mut() = url;
    }
}

#[derive(Debug)]
pub enum SendRequestError {
    EndpointNotSet,
    NoScheme,
    NoCommonProtocol,
    UnsupportedScheme(String),
    NoHost,
    ServerNameParseError(InvalidDnsNameError),
    UnsupportedServerNameType,
    Websocket(JsValue),
    TlsConnect(IoError),
    ConnectionNotReady,
    HttpHandshake(hyper::Error),
    Hyper(hyper::Error),
}

impl std::fmt::Display for SendRequestError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SendRequestError::EndpointNotSet => write!(f, "Endpoint not set. Please call init before sending requests"),
            SendRequestError::NoScheme => write!(f, "No scheme in URI"),
            SendRequestError::NoCommonProtocol => write!(f, "The server and client do not have a common protocol"),
            SendRequestError::UnsupportedScheme(scheme) => write!(f, "Unsupported scheme: {scheme}"),
            SendRequestError::NoHost => write!(f, "No host in URI"),
            SendRequestError::ServerNameParseError(e) => write!(f, "Error parsing server name: {e}"),
            SendRequestError::Websocket(e) => write!(f, "Error opening websocket: {e:?}"),
            SendRequestError::UnsupportedServerNameType => write!(f, "Unsupported server name type"),
            SendRequestError::TlsConnect(e) => write!(f, "Error connecting to TLS server: {e}"),
            SendRequestError::ConnectionNotReady => write!(f, "Connection not ready"),
            SendRequestError::HttpHandshake(e) => write!(f, "Error in HTTP handshake: {e}"),
            SendRequestError::Hyper(e) => write!(f, "Hyper error: {e}"),
        }
    }
}

impl std::error::Error for SendRequestError {}

fn get_server(uri: &Uri) -> Result<(String, ServerName<'static>), SendRequestError> {
    let port = match uri.port_u16() {
        Some(port) => port,
        None => match uri.scheme_str() {
            Some("http") => 80,
            Some("https") => 443,
            Some(any) => return Err(SendRequestError::UnsupportedScheme(any.to_owned())),
            None => return Err(SendRequestError::NoScheme),
        }
    };
    let host = uri.authority().map(|a| a.host().to_owned()).ok_or(SendRequestError::NoHost)?;
    let server_name = ServerName::try_from(host).map_err(SendRequestError::ServerNameParseError)?;
    let multiaddr = match &server_name {
        ServerName::DnsName(domain) => format!("dnsaddr/{}/tcp/{port}", domain.as_ref()),
        ServerName::IpAddress(RustlsIpAddr::V4(ip)) => {
            let [a, b, c, d] = ip.as_ref();
            format!("ip4/{a}.{b}.{c}.{d}/tcp/{port}")
        },
        ServerName::IpAddress(RustlsIpAddr::V6(ip)) => {
            let array: &[u8; 16] = ip.as_ref();
            let array: &[u16; 8] = unsafe { &*(array as *const _ as *const _) };
            let [a, b, c, d, e, f, g, h] = array;
            format!("ip6/{a}:{b}:{c}:{d}:{e}:{f}:{g}:{h}/tcp/{port}")
        },
        _ => return Err(SendRequestError::UnsupportedServerNameType),
    };
    
    Ok((multiaddr, server_name))
}


impl Pool {
    async fn send_request_new_stream(&self, request: Request<MantalonBody>)  -> Result<Response<Incoming>, SendRequestError> {
        let uri = request.uri();
        let (multiaddr, server_name) = get_server(uri)?;
        debug!("Opening connection to {}", multiaddr);

        // Get the endpoint
        let mantalon_endpoint = MANTALON_ENDPOINT.0.borrow();
        if mantalon_endpoint.is_empty() {
            return Err(SendRequestError::EndpointNotSet);
        }
        let ws_url = match mantalon_endpoint.ends_with('/') {
            true => format!("{mantalon_endpoint}{multiaddr}"),
            false => format!("{mantalon_endpoint}/{multiaddr}"),
        };
        std::mem::drop(mantalon_endpoint);

        // Open the websocket
        let connections2 = Rc::clone(&self.connections);
        let multiaddr2 = multiaddr.clone();
        let on_close = || spawn_local(async move { connections2.write().await.remove(&multiaddr2); });
        let websocket = WebSocket::new(&ws_url).map_err(SendRequestError::Websocket)?;

        // Wrap the websocket
        let websocket = WrappedWebSocket::new(websocket, on_close);
        websocket.ready().await;
        if websocket.ready_state() != WebSocket::OPEN {
            return Err(SendRequestError::Websocket(JsValue::from_str("Websocket not open")));
        }

        let mut request_sender = if uri.scheme().map(|s| s.as_str()).unwrap_or_default() == "https" {
            // Encrypt stream :)
            let mut root_cert_store = RootCertStore::empty();
            root_cert_store.extend(webpki_roots::TLS_SERVER_ROOTS.iter().cloned());
            let mut config = ClientConfig::builder_with_protocol_versions(tokio_rustls::rustls::ALL_VERSIONS)
                .with_root_certificates(root_cert_store)
                .with_no_client_auth();
            config.alpn_protocols.push(b"h2".to_vec());
            config.alpn_protocols.push(b"http/1.1".to_vec());
            let connector = TlsConnector::from(Arc::new(config));
            let stream = connector.connect(server_name, websocket).await.map_err(SendRequestError::TlsConnect)?;
            let alpn_protocol = stream.get_ref().1.alpn_protocol().map(|s| s.to_vec());
            let stream = TokioIo::new(stream);
            
            match alpn_protocol.as_deref() {
                Some(b"http/1.1") => SendRequest::new_h1(stream).await.map_err(SendRequestError::HttpHandshake)?,
                Some(b"h2") => SendRequest::new_h2(stream).await.map_err(SendRequestError::HttpHandshake)?,
                _ => return Err(SendRequestError::NoCommonProtocol),
            }
        } else {
            // Don't encrypt stream :(
            let stream = TokioIo::new(websocket);
            SendRequest::new_h1(stream).await.map_err(SendRequestError::HttpHandshake)?
        };

        // Store the connection
        let request_sender2 = request_sender.clone();
        if let Ok(mut connections) = self.connections.try_write() {
            connections.insert(multiaddr.clone(), request_sender2);
        } else {
            let connections = Rc::clone(&self.connections);
            spawn_local(async move {
                connections.write().await.insert(multiaddr, request_sender2);
            });
        }

        // Send the request
        request_sender.ready().await.map_err(|_| SendRequestError::ConnectionNotReady)?;
        request_sender.send_request(request).await.map_err(SendRequestError::Hyper)
    }

    pub async fn send_request(&self, request: Request<MantalonBody>) -> Result<Response<Incoming>, SendRequestError> {
        let uri = request.uri();
        let (multiaddr, _) = get_server(uri)?;

        let connections = self.connections.read().await;
        match connections.get(&multiaddr) {
            Some(t) => {
                debug!("Reusing connection to {}", multiaddr);
                
                let mut conn = SendRequest::clone(t);
                std::mem::drop(connections);
                let ready = conn.ready().await;
                if ready.is_err() {
                    return self.send_request_new_stream(request).await;
                }
                conn.send_request(request).await.map_err(SendRequestError::Hyper)
            }
            None => {
                drop(connections);
                self.send_request_new_stream(request).await
            }
        }
    }
}
