use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::*;
use web_sys::*;

macro_rules! log {
    ( $( $t:tt )* ) => {
        web_sys::console::log_1(&format!( $( $t )* ).into());
    }
}

macro_rules! error {
    ( $( $t:tt )* ) => {
        web_sys::console::error_1(&format!( $( $t )* ).into());
    }
}

#[wasm_bindgen]
pub async fn test() {
    log!("Hello from Rust!");

    let websocket = match WebSocket::new("ws://localhost:8080/connect/ip4/93.184.215.14/tcp/80") {
        Ok(websocket) => websocket,
        Err(err) => {
            error!("Could not open websocket to mantalon proxy server: {:?}", err);
            return;
        }
    };
    log!("Websocket: {:?}", websocket);
}
