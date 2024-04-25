use std::{cell::RefCell, collections::VecDeque, io::{Error as IoError, ErrorKind as IoErrorKind}, pin::Pin, rc::Rc, task::{Context, Poll, Waker}};
use js_sys::Uint8Array;
use tokio::io::{AsyncWrite, AsyncWriteExt, AsyncRead, AsyncReadExt};
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

// async fn blob_into_bytes(blob: Blob) -> Vec<u8> {
//     let array_buffer_promise: JsFuture = blob
//         .array_buffer()
//         .into();
//
//     let array_buffer: JsValue = array_buffer_promise
//         .await
//         .expect("Could not get ArrayBuffer from file");
//
//     js_sys::Uint8Array
//         ::new(&array_buffer)
//         .to_vec()
// }

fn blob_into_bytes(blob: Blob) -> Vec<u8> {
    let file_reader = FileReader::new().unwrap();
    file_reader.read_as_array_buffer(&blob).unwrap();

    let array = Uint8Array::new(&file_reader.result().unwrap());
    array.to_vec() // OPTIM
}

struct WrappedWebSocket {
    buffer: Rc<RefCell<VecDeque<u8>>>,
    read_waker: Option<Waker>,
    on_message: Closure<dyn FnMut(MessageEvent)>,
    ws: WebSocket
}

impl WrappedWebSocket {
    fn new(ws: WebSocket) -> Self {
        let buffer = Rc::new(RefCell::new(VecDeque::new()));

        // Create message receiver
        let buffer2 = Rc::clone(&buffer);
        let on_message = Closure::wrap(Box::new(move |event: MessageEvent| {
            if let Ok(blob) = event.data().dyn_into::<Blob>() {
                let data = blob_into_bytes(blob);
                buffer2.borrow_mut().extend(data);
            }
        }) as Box<dyn FnMut(MessageEvent)>);
        ws.set_onmessage(Some(on_message.as_ref().unchecked_ref()));

        WrappedWebSocket {
            buffer,
            read_waker: None,
            on_message,
            ws
        }
    }
}

impl AsyncWrite for WrappedWebSocket {
    fn poll_write(self: Pin<&mut Self>, _cx: &mut Context<'_>, buf: &[u8]) -> Poll<Result<usize, IoError>> {
        match self.ws.send_with_u8_array(buf) {
            Ok(_) => Poll::Ready(Ok(buf.len())),
            Err(err) => {
                error!("Error sending data over websocket: {:?}", err);
                Poll::Ready(Err(IoError::new(IoErrorKind::Other, "Error sending data over websocket")))
            }
        }
    }

    fn poll_flush(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<Result<(), IoError>> {
        Poll::Ready(Ok(()))   
    }

    fn poll_shutdown(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<Result<(), IoError>> {
        match self.ws.close() {
            Ok(_) => Poll::Ready(Ok(())),
            Err(err) => {
                error!("Error closing websocket: {:?}", err);
                Poll::Ready(Err(IoError::new(IoErrorKind::Other, "Error closing websocket")))
            }
        }
    }
}

impl AsyncRead for WrappedWebSocket {
    fn poll_read(mut self: Pin<&mut Self>, cx: &mut Context<'_>, buf: &mut tokio::io::ReadBuf<'_>) -> Poll<Result<(), IoError>> {
        self.read_waker = Some(cx.waker().clone()); // TODO optim

        let mut buffer = self.buffer.borrow_mut();
        while buf.remaining() > 0 {
            if let Some(byte) = buffer.pop_front() { // OPTIM
                buf.put_slice(&[byte]);
            } else {
                return Poll::Pending;
            }
        }
        
        Poll::Ready(Ok(()))
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

    let websocket = WrappedWebSocket::new(websocket);
}
