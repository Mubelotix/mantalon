use std::{collections::HashMap, future::Future, io::Error as IoError, rc::Rc};
use bytes::Bytes;
use http::{Request, Response, Uri};
use hyper::{client::conn::http2::SendRequest, rt::bounds::Http2ClientConnExec};
use tokio::sync::RwLock;
use tokio_rustls::rustls::pki_types::InvalidDnsNameError;
use crate::*;
use lazy_static::lazy_static;
use tokio::sync::Mutex;

lazy_static!{
    pub static ref POOL: Pool = Pool::default();
}

#[derive(Default)]
pub struct Pool {
    #[allow(clippy::type_complexity)]
    connections: Rc<RwLock<HashMap<String, Rc<Mutex<SendRequest<MantalonBody>>>>>>
}

unsafe impl Send for Pool {}
unsafe impl Sync for Pool {}

#[derive(Debug)]
pub enum SendRequestError {
    NoScheme,
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
            SendRequestError::NoScheme => write!(f, "No scheme in URI"),
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

#[derive(Clone)]
struct WasmExecutor;

impl<Fut> hyper::rt::Executor<Fut> for WasmExecutor
    where
    Fut: Future + Send + 'static,
    Fut::Output: Send + 'static,
{
    fn execute(&self, fut: Fut) {
        spawn_local(async move {fut.await;});
    }
}

unsafe impl Send for WasmExecutor {}
unsafe impl Sync for WasmExecutor {}

impl Pool {
    pub async fn send_request(&self, request: Request<MantalonBody>) -> Result<Response<Incoming>, SendRequestError> {
        let uri = request.uri();
        let (multiaddr, server_name) = get_server(uri)?;

        match self.connections.read().await.get(&multiaddr).map(Rc::clone) {
            Some(t) => {
                debug!("Reusing connection to {}", multiaddr);
                
                let mut conn = t.lock().await;
                conn.ready().await.map_err(|_| SendRequestError::ConnectionNotReady)?;
                conn.send_request(request).await.map_err(SendRequestError::Hyper)
            }
            None => {
                debug!("Opening connection to {}", multiaddr);

                // Open the websocket
                let ws_url = format!("ws://localhost:8000/mantalon-connect/{}", multiaddr);
                let connections2 = Rc::clone(&self.connections);
                let multiaddr2 = multiaddr.clone();
                let on_close = || spawn_local(async move { connections2.write().await.remove(&multiaddr2); });
                let websocket = WebSocket::new(&ws_url).map_err(SendRequestError::Websocket)?;
                let websocket = WrappedWebSocket::new(websocket, on_close);
                websocket.ready().await;

                let request_sender = if uri.scheme().map(|s| s.as_str()).unwrap_or_default() == "https" {
                    // Encrypt stream :)
                    let mut root_cert_store = RootCertStore::empty();
                    root_cert_store.extend(webpki_roots::TLS_SERVER_ROOTS.iter().cloned());
                    let mut config = ClientConfig::builder()
                        .with_root_certificates(root_cert_store)
                        .with_no_client_auth();
                    config.alpn_protocols.push(b"h2".to_vec());
                    config.alpn_protocols.push(b"http/1.1".to_vec());
                    let connector = TlsConnector::from(Arc::new(config));
                    let stream = connector.connect(server_name, websocket).await.map_err(SendRequestError::TlsConnect)?;
                    let stream = TokioIo::new(stream);
                    let (request_sender, connection) = conn::http2::handshake(WasmExecutor, stream).await.map_err(SendRequestError::HttpHandshake)?;
                
                    // Spawn a task to poll the connection and drive the HTTP state
                    spawn_local(async move {
                        if let Err(e) = connection.await {
                            error!("Error in connection: {}", e);
                        }
                    });

                    request_sender
                } else {
                    // Don't encrypt stream :(
                    let stream = TokioIo::new(websocket);
                    let (request_sender, connection) = conn::http2::handshake(WasmExecutor, stream).await.map_err(SendRequestError::HttpHandshake)?;
                
                    // Spawn a task to poll the connection and drive the HTTP state
                    spawn_local(async move {
                        if let Err(e) = connection.await {
                            error!("Error in connection: {}", e);
                        }
                    });

                    request_sender
                };

                // Store the connection
                let request_sender = Rc::new(Mutex::new(request_sender));
                let request_sender2 = Rc::clone(&request_sender);
                let mut request_sender = request_sender.try_lock().expect("a mutex we just created can't be initially locked");
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
        }
    }
}
