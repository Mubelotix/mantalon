use std::{sync::Arc, time::Duration};
use js_sys::Promise;
use tokio_rustls::{rustls::{pki_types::ServerName, ClientConfig, RootCertStore}, TlsConnector};
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::*;
use web_sys::*;
use bytes::Bytes;
use http::{Request, StatusCode};
use http_body_util::{BodyExt, Empty};
use hyper::{body::{Body, Incoming}, client::conn};

use crate::compat::TokioIo;

mod compat;
mod websocket;
use websocket::*;

#[macro_export]
macro_rules! log {
    ( $( $t:tt )* ) => {
        web_sys::console::log_1(&format!( $( $t )* ).into());
    }
}

#[macro_export]
macro_rules! error {
    ( $( $t:tt )* ) => {
        web_sys::console::error_1(&format!( $( $t )* ).into());
    }
}

async fn read_body(mut body: Incoming) -> Option<Vec<u8>> {
    let mut body_bytes = Vec::new();
    while !body.is_end_stream() {
        let chunk = body.frame().await.unwrap();
        match chunk {
            Ok(chunk) => match chunk.into_data() {
                Ok(data) => body_bytes.extend_from_slice(&data),
                Err(e) => {
                    error!("Received non-data frame: {:?}", e);
                    return None;
                }
            },
            Err(err) => {
                error!("Error reading chunk: {:?}", err);
                return None;
            }
        }
    }
    Some(body_bytes)
}

pub async fn sleep(duration: Duration) {
    JsFuture::from(Promise::new(&mut |yes, _| {
        window()
            .unwrap()
            .set_timeout_with_callback_and_timeout_and_arguments_0(
                &yes,
                duration.as_millis() as i32,
            )
            .unwrap();
    })).await.unwrap();
}

#[wasm_bindgen]
pub async fn test() {
    std::panic::set_hook(Box::new(|panic_info| {
        if let Some(s) = panic_info.payload().downcast_ref::<&str>() {
            if let Some(location) = panic_info.location() {
                error!("mantalon panicked at {}:{}, {s}", location.file(), location.line());
            } else {
                error!("mantalon panicked, {s}");
            }
        } else if let Some(s) = panic_info.payload().downcast_ref::<String>() {
            if let Some(location) = panic_info.location() {
                error!("mantalon panicked at {}:{}, {s}", location.file(), location.line());
            } else {
                error!("mantalon panicked, {s}");
            }
        } else {
            error!("panic occurred");
        }
    }));

    log!("Hello from Rust!");

    let websocket = match WebSocket::new("ws://localhost:8080/connect/ip4/93.184.215.14/tcp/443") {
        Ok(websocket) => WrappedWebSocket::new(websocket),
        Err(err) => {
            error!("Could not open websocket to mantalon proxy server: {:?}", err);
            return;
        }
    };
    websocket.ready().await;

    let mut root_cert_store = RootCertStore::empty();
    root_cert_store.extend(webpki_roots::TLS_SERVER_ROOTS.iter().cloned());
    let config = ClientConfig::builder()
        .with_root_certificates(root_cert_store)
        .with_no_client_auth();
    let connector = TlsConnector::from(Arc::new(config));
    let dnsname = ServerName::try_from("example.com").unwrap();
    let stream = connector.connect(dnsname, websocket).await.unwrap();
    log!("TLS connection established");

    sleep(Duration::from_secs(1)).await;

    let stream = TokioIo::new(stream);
    let (mut request_sender, connection) = conn::http1::handshake(stream).await.unwrap();
    log!("HTTP connection established");

    // spawn a task to poll the connection and drive the HTTP state
    wasm_bindgen_futures::spawn_local(async move {
        if let Err(e) = connection.await {
            error!("Error in connection: {}", e);
        }
    });

    let request = Request::builder()
        // We need to manually add the host header because SendRequest does not
        .header("Host", "example.com")
        .method("GET")
        .body(Empty::<Bytes>::new()).unwrap();

    log!("Sending request: {:?}", request);
    request_sender.ready().await.unwrap();
    let response = request_sender.send_request(request).await.unwrap();
    log!("Response: {:?}", response);
    assert!(response.status() == StatusCode::OK);
    let body = read_body(response.into_body()).await.unwrap_or_default();
    log!("Body: {:?}", String::from_utf8(body).unwrap());

    // let request = Request::builder()
    //     .header("Host", "example.com")
    //     .method("GET")
    //     .body(Empty::<Bytes>::new()).unwrap();

    // log!("Sending request: {:?}", request);
    // request_sender.ready().await.unwrap();
    // let response = request_sender.send_request(request).await.unwrap();
    // log!("Response: {:?}", response);
    // assert!(response.status() == StatusCode::OK);
    // let body = read_body(response.into_body()).await.unwrap_or_default();
    // log!("Body: {:?}", String::from_utf8(body).unwrap());
}
