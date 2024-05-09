#![allow(clippy::map_clone)]

use std::sync::Arc;
use tokio_rustls::{rustls::{pki_types::{ServerName, IpAddr as RustlsIpAddr}, ClientConfig, RootCertStore}, TlsConnector};
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::*;
use web_sys::*;
use bytes::Bytes;
use http_body_util::{BodyExt, Empty};
use hyper::{body::{Body, Incoming}, client::conn};

use crate::compat::TokioIo;

mod compat;
mod exports;
mod websocket;
use websocket::*;
mod pool;
use pool::*;
mod manifest;
use manifest::*;

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

pub async fn read_body(mut body: Incoming) -> Option<Vec<u8>> {
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
            None => {
                debug!("Unexpected end of stream");
                break;
            }
        }
    }
    Some(body_bytes)
}

pub async fn proxied_fetch(request: http::Request<Empty<Bytes>>) -> Result<http::Response<Incoming>, SendRequestError> {
    debug!("Request: {request:?}");

    let response = POOL.send_request(request).await.map_err(|e| {
        error!("Error sending request: {:?}", e);
        e
    })?;
    
    debug!("Response: {response:?}");
    
    Ok(response)
}
