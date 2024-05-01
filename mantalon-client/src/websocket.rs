use std::{cell::RefCell, collections::VecDeque, future::Future, io::{Error as IoError, ErrorKind as IoErrorKind}, pin::Pin, rc::Rc, task::{Context, Poll, Waker}};
use tokio::io::{AsyncWrite, AsyncRead};
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::*;
use web_sys::*;
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
    open_waker: Rc<RefCell<Option<Waker>>>,
    _on_open: Closure<dyn FnMut(Event)>,
    _on_close: Closure<dyn FnMut(Event)>,
    _on_error: Closure<dyn FnMut(Event)>,
    _on_message: Closure<dyn FnMut(MessageEvent)>,
    ws: WebSocket
}

impl WrappedWebSocket {
    pub fn new(ws: WebSocket, on_close: impl FnOnce() + 'static) -> Self {
        let buffer = Rc::new(RefCell::new(VecDeque::new()));
        let read_waker: Rc<RefCell<Option<Waker>>> = Rc::new(RefCell::new(None));
        let open_waker: Rc<RefCell<Option<Waker>>> = Rc::new(RefCell::new(None));

        // Create open listener
        let open_waker2 = Rc::clone(&open_waker);
        let on_open = Closure::wrap(Box::new(move |_| {
            if let Some(waker) = open_waker2.borrow_mut().as_ref() {
                waker.wake_by_ref();
            }
        }) as Box<dyn FnMut(Event)>);
        ws.set_onopen(Some(on_open.as_ref().unchecked_ref()));

        // Create close listener
        let mut on_close_inner = Some(on_close);
        let on_close = Closure::wrap(Box::new(move |_| {
            error!("Websocket closed");
            if let Some(on_close) = on_close_inner.take() {
                on_close();
            }
        }) as Box<dyn FnMut(Event)>);
        ws.set_onclose(Some(on_close.as_ref().unchecked_ref()));

        // Create error listener
        let on_error = Closure::wrap(Box::new(move |_| {
            error!("Websocket error");
        }) as Box<dyn FnMut(Event)>);
        ws.set_onerror(Some(on_error.as_ref().unchecked_ref()));

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
            open_waker,
            _on_open: on_open,
            _on_close: on_close,
            _on_error: on_error,
            _on_message: on_message,
            ws
        }
    }
}

pub struct WebsocketReadyFut<'a>(&'a WrappedWebSocket);

impl<'a> Future for WebsocketReadyFut<'a> {
    type Output = ();

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<()> {
        if self.0.ws.ready_state() != WebSocket::CONNECTING {
            Poll::Ready(())
        } else {
            *self.0.open_waker.borrow_mut() = Some(cx.waker().clone());
            Poll::Pending
        }
    }
}

impl WrappedWebSocket {
    pub fn ready(&self) -> WebsocketReadyFut {
        WebsocketReadyFut(self)
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

impl Drop for WrappedWebSocket {
    fn drop(&mut self) {
        self.ws.set_onclose(None);
        self.ws.set_onerror(None);
        self.ws.set_onmessage(None);
        self.ws.set_onopen(None);
        self.ws.close().unwrap();
    }
}
