use std::{collections::HashMap, rc::Rc};

use bytes::Bytes;
use http::{Request, Response, Uri};
use http_body_util::Empty;
use hyper::client::conn::http1::SendRequest;
use tokio::sync::RwLock;
use crate::*;
use lazy_static::lazy_static;
use tokio::sync::Mutex;

lazy_static!{
    pub static ref POOL: Pool = Pool::default();
}

#[derive(Default)]
pub struct Pool {
    connections: Rc<RwLock<HashMap<String, Rc<Mutex<SendRequest<Empty<Bytes>>>>>>>
}

unsafe impl Send for Pool {}
unsafe impl Sync for Pool {}

fn get_server(uri: &Uri) -> Result<(String, ServerName<'static>), ()> {
    let port = match uri.port_u16() {
        Some(port) => port,
        None => match uri.scheme_str() {
            Some("http") => 80,
            Some("https") => 443,
            _ => {
                error!("Unsupported scheme: {:?}", uri.scheme());
                return Err(());
            }
        }
    };
    let server_name = ServerName::try_from(uri.authority().map(|a| a.host().to_owned()).unwrap()).unwrap();
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
        other => {
            error!("Unsupported server name type: {:?}", other);
            return Err(());
        }
    };
    
    Ok((multiaddr, server_name))
}

impl Pool {
    pub async fn send_request(&self, request: Request<Empty<Bytes>>) -> Result<Response<Incoming>, hyper::Error> {
        let uri = request.uri();
        let (multiaddr, server_name) = get_server(uri).unwrap();

        match self.connections.read().await.get(&multiaddr).map(Rc::clone) {
            Some(t) => {
                debug!("Reusing connection to {}", multiaddr);
                
                let mut conn = t.lock().await;
                conn.ready().await.unwrap();
                conn.send_request(request).await
            }
            None => {
                debug!("Opening connection to {}", multiaddr);

                // Open the websocket
                let ws_url = format!("ws://localhost:8000/mantalon-connect/{}", multiaddr);
                let connections2 = Rc::clone(&self.connections);
                let multiaddr2 = multiaddr.clone();
                let on_close = || spawn_local(async move { connections2.write().await.remove(&multiaddr2); });
                let websocket = match WebSocket::new(&ws_url) {
                    Ok(websocket) => WrappedWebSocket::new(websocket, on_close),
                    Err(err) => {
                        error!("Could not open websocket to mantalon proxy server: {:?}", err);
                        todo!()
                        //return Err(());
                    }
                };
                websocket.ready().await;

                // Encrypt stream
                let mut root_cert_store = RootCertStore::empty();
                root_cert_store.extend(webpki_roots::TLS_SERVER_ROOTS.iter().cloned());
                let config = ClientConfig::builder()
                    .with_root_certificates(root_cert_store)
                    .with_no_client_auth();
                let connector = TlsConnector::from(Arc::new(config));
                let stream = connector.connect(server_name, websocket).await.unwrap();
                let stream = TokioIo::new(stream);
                let (request_sender, connection) = conn::http1::handshake(stream).await.unwrap();

                // Spawn a task to poll the connection and drive the HTTP state
                spawn_local(async move {
                    if let Err(e) = connection.await {
                        error!("Error in connection: {}", e);
                    }
                });

                // Store the connection
                let request_sender = Rc::new(Mutex::new(request_sender));
                let request_sender2 = Rc::clone(&request_sender);
                let mut request_sender = request_sender.try_lock().unwrap();
                if let Ok(mut connections) = self.connections.try_write() {
                    connections.insert(multiaddr.clone(), request_sender2);
                } else {
                    let connections = Rc::clone(&self.connections);
                    spawn_local(async move {
                        connections.write().await.insert(multiaddr, request_sender2);
                    });
                }

                // Send the request
                request_sender.ready().await.unwrap();
                request_sender.send_request(request).await
            }
        }
    }
}
