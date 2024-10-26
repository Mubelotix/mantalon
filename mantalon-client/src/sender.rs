use http::Request;
use hyper::{client::conn::{http1::SendRequest as SendRequest1, http2::SendRequest as SendRequest2}, rt::{Read, Write}};
use tokio::sync::Mutex;
use crate::*;

/// A protocol-agnostic request sender.
pub enum SendRequest {
    H1(Arc<Mutex<SendRequest1<MantalonBody>>>),
    H2(SendRequest2<MantalonBody>),
}

impl SendRequest {
    /// Handshakes with a server and returns a new `SendRequest`.
    pub async fn new_h1<T: Read + Write + Unpin + Send + Sync + 'static>(stream: T) -> Result<Self, hyper::Error> {
        let (request_sender, connection) = conn::http1::handshake(stream).await?;

        spawn_local(async move {
            if let Err(e) = connection.await {
                error!("Error in connection: {}", e);
            }
        });

        Ok(SendRequest::H1(Arc::new(Mutex::new(request_sender))))
    }

    /// Handshakes with a server and returns a new `SendRequest`.
    pub async fn new_h2<T: Read + Write + Unpin + Send + Sync + 'static>(stream: T) -> Result<Self, hyper::Error> {
        let (request_sender, connection) = conn::http2::Builder::new(WasmExecutor)
            .max_concurrent_reset_streams(0)
            .handshake(stream).await?;

        spawn_local(async move {
            if let Err(e) = connection.await {
                error!("Error in connection: {}", e);
            }
        });
        
        Ok(SendRequest::H2(request_sender))
    }

    /// Waits for the connection to be ready.
    pub async fn ready(&mut self) -> std::result::Result<(), hyper::Error> {
        match self {
            SendRequest::H1(r) => r.lock().await.ready().await,
            SendRequest::H2(r) => r.ready().await,
        }
    }

    /// Sends a request.
    pub async fn send_request(&mut self, mut req: Request<MantalonBody>) -> Result<http::Response<Incoming>, hyper::Error> {
        match self {
            SendRequest::H1(r) => {
                if let Some(authority) = req.uri().authority() {
                    if let Ok(host) = authority.host().parse() {
                        req.headers_mut().insert("host", host);
                    }
                }

                r.lock().await.send_request(req).await
            },
            SendRequest::H2(r) => r.send_request(req).await,
        }
    }
}

impl Clone for SendRequest {
    fn clone(&self) -> Self {
        match self {
            SendRequest::H1(r) => SendRequest::H1(Arc::clone(r)),
            SendRequest::H2(r) => SendRequest::H2(r.clone()),
        }
    }
}
