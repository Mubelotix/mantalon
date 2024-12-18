#![allow(clippy::map_clone)]
#![allow(clippy::await_holding_refcell_ref)] // Because of false positives

use std::{sync::Arc, time::Duration};
use js_sys::{global, Promise};
use tokio_rustls::{rustls::{pki_types::{ServerName, IpAddr as RustlsIpAddr}, ClientConfig, RootCertStore}, TlsConnector};
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::*;
use web_sys::*;
use http_body_util::BodyExt;
use hyper::{body::{Body, Incoming}, client::conn};
use crate::compat::TokioIo;

mod compat;
mod exports;
mod websocket;
use websocket::*;
mod pool;
use pool::*;
mod executor;
pub use executor::*;
mod sender;
pub use sender::*;
mod body;
pub use body::*;

#[macro_export]
macro_rules! log {
    ( $( $t:tt )* ) => {
        web_sys::console::log_1(&format!( $( $t )* ).into())
    }
}

#[macro_export]
macro_rules! error {
    ( $( $t:tt )* ) => {
        web_sys::console::error_1(&format!( $( $t )* ).into())
    }
}

#[macro_export]
macro_rules! debug {
    ( $( $t:tt )* ) => {
        web_sys::console::debug_1(&format!( $( $t )* ).into())
    }
}

pub fn now() -> i64 {
    (js_sys::Date::new_0().get_time() / 1000.0) as i64
}

pub async fn read_entire_body(mut body: Incoming) -> Option<Vec<u8>> {
    let mut body_bytes = Vec::new();
    while !body.is_end_stream() {
        let chunk = body.frame().await;
        match chunk {
            Some(Ok(chunk)) => match chunk.into_data() {
                Ok(data) => body_bytes.extend_from_slice(&data),
                Err(e) => {
                    error!("Received non-data frame: {:?}", e);
                    return None;
                }
            },
            Some(Err(err)) => {
                error!("Error reading chunk: {:?}", err);
                return None;
            },
            None => break,
        }
    }
    Some(body_bytes)
}

pub async fn proxied_fetch(request: http::Request<MantalonBody>) -> Result<http::Response<Incoming>, SendRequestError> {
    debug!("Request: {request:?}");

    let response = POOL.send_request(request).await.map_err(|e| {
        error!("Error sending request: {:?}", e);
        e
    })?;
    
    debug!("Response: {response:?}");
    
    Ok(response)
}

pub async fn sleep(duration: Duration) {
    JsFuture::from(Promise::new(&mut |yes, _| {
        global()
            .dyn_into::<ServiceWorkerGlobalScope>()
            .unwrap()
            .set_timeout_with_callback_and_timeout_and_arguments_0(
                &yes,
                duration.as_millis() as i32,
            )
            .unwrap();
    })).await.unwrap();
}
