use std::{cell::RefCell, collections::VecDeque, io::{Error as IoError, ErrorKind as IoErrorKind}, pin::Pin, rc::Rc, task::{Context, Poll, Waker}, time::Duration};
use js_sys::{Promise, Uint8Array};
use tokio::io::{AsyncWrite, AsyncWriteExt, AsyncRead, AsyncReadExt};
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::*;
use web_sys::*;
use bytes::Bytes;
use http::{Request, StatusCode};
use http_body_util::{BodyExt, Empty};
use hyper::{body::{Body, Incoming}, client::conn};

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

struct WrappedWebSocket {
    buffer: Rc<RefCell<VecDeque<u8>>>,
    read_waker: Rc<RefCell<Option<Waker>>>,
    on_message: Closure<dyn FnMut(MessageEvent)>,
    ws: WebSocket
}

impl WrappedWebSocket {
    fn new(ws: WebSocket) -> Self {
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

impl hyper::rt::Write for WrappedWebSocket {
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

impl hyper::rt::Read for WrappedWebSocket {
    fn poll_read(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
        mut buf: hyper::rt::ReadBufCursor<'_>,
    ) -> Poll<Result<(), std::io::Error>> {
        *self.read_waker.borrow_mut() = Some(cx.waker().clone()); // TODO optim

        let mut n = 0;
        unsafe {
            let mut buf = tokio::io::ReadBuf::uninit(buf.as_mut());
            let mut buffer = self.buffer.borrow_mut();
            if buf.remaining() == 0 {
                return Poll::Ready(Ok(()));
            }
            while buf.remaining() > 0 {
                if let Some(byte) = buffer.pop_front() { // OPTIM
                    buf.put_slice(&[byte]);
                    n += 1;
                } else {
                    break;
                }
            }
        }

        if n == 0 {
            return Poll::Pending;
        }

        unsafe {
            buf.advance(n);
        }
        Poll::Ready(Ok(()))
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

    let websocket = match WebSocket::new("ws://localhost:8080/connect/ip4/93.184.215.14/tcp/80") {
        Ok(websocket) => websocket,
        Err(err) => {
            error!("Could not open websocket to mantalon proxy server: {:?}", err);
            return;
        }
    };
    log!("Websocket: {:?}", websocket);

    let websocket = WrappedWebSocket::new(websocket);

    sleep(Duration::from_secs(1)).await;

    let (mut request_sender, connection) = conn::http1::handshake(websocket).await.unwrap();
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

    let request = Request::builder()
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
}
