use std::{cell::RefCell, collections::VecDeque, io::{Error as IoError, ErrorKind as IoErrorKind}, pin::Pin, rc::Rc, task::{Context, Poll, Waker}};
use tokio::io::{AsyncWrite, AsyncRead};
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::*;
use web_sys::*;
use http_body_util::BodyExt;
use hyper::body::{Body, Incoming};
use crate::*;

async fn blob_into_bytes(blob: Blob) -> Vec<u8> {
    let array_buffer_promise: JsFuture = blob
        .array_buffer()
        .into();

    let array_buffer: JsValue = array_buffer_promise
        .await
        .expect("Could not get ArrayBuffer from file");

    js_sys::Uint8Array
        ::new(&array_buffer)
        .to_vec()
}

// fn blob_into_bytes(blob: Blob) -> Vec<u8> {
//     let file_reader = FileReader::new().unwrap();
//     file_reader.read_as_array_buffer(&blob).unwrap();

//     let array = Uint8Array::new(&file_reader.result().unwrap());
//     array.to_vec() // OPTIM
// }

pub struct WrappedWebSocket {
    buffer: Rc<RefCell<VecDeque<u8>>>,
    read_waker: Rc<RefCell<Option<Waker>>>,
    on_message: Closure<dyn FnMut(MessageEvent)>,
    ws: WebSocket
}

impl WrappedWebSocket {
    pub fn new(ws: WebSocket) -> Self {
        let buffer = Rc::new(RefCell::new(VecDeque::new()));
        let read_waker: Rc<RefCell<Option<Waker>>> = Rc::new(RefCell::new(None));

        // Create message receiver
        let buffer2 = Rc::clone(&buffer);
        let read_waker2 = Rc::clone(&read_waker);
        let on_message = Closure::wrap(Box::new(move |event: MessageEvent| {
            if let Ok(blob) = event.data().dyn_into::<Blob>() {
                let buffer3 = Rc::clone(&buffer2);
                let read_waker3 = Rc::clone(&read_waker2);
                spawn_local(async move {
                    let data = blob_into_bytes(blob).await;
                    buffer3.borrow_mut().extend(data);
                    if let Some(waker) = read_waker3.borrow_mut().as_ref() {
                        waker.wake_by_ref();
                    }
                });
            } else {
                error!("Received non-blob message from websocket");
            }
        }) as Box<dyn FnMut(MessageEvent)>);
        ws.set_onmessage(Some(on_message.as_ref().unchecked_ref()));

        WrappedWebSocket {
            buffer,
            read_waker,
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
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        buf: &mut tokio::io::ReadBuf<'_>,
    ) -> Poll<std::io::Result<()>> {
        *self.read_waker.borrow_mut() = Some(cx.waker().clone()); // TODO optim
        let mut n = 0;
        let mut buffer = self.buffer.borrow_mut();
        while buf.remaining() > 0 {
            if let Some(byte) = buffer.pop_front() { // OPTIM
                buf.put_slice(&[byte]);
                n += 1;
            } else {
                break;
            }
        }
        if n == 0 {
            return Poll::Pending;
        }
        Poll::Ready(Ok(()))
    }
}
