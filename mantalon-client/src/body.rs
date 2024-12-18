use std::{collections::VecDeque, fmt, future::Future, pin::Pin, task::{Context, Poll}};
use crate::*;
use hyper::body::Frame;
use js_sys::{encode_uri_component, Object, Reflect, Uint8Array};
use pin_project_lite::pin_project;

pin_project! {
    /// A body that can either be empty or a javascript readable stream.
    #[project = MantalonBodyProj]
    pub enum MantalonBody {
        Empty,
        FormData { form: Vec<(String, String)> },
        Known { data: Option<VecDeque<u8>> },
        ReadableStream { reader: ReadableStreamDefaultReader, #[pin] fut: JsFuture, },
    }
}

impl fmt::Debug for MantalonBody {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            MantalonBody::Empty => f.debug_struct("MantalonBody::Empty").finish(),
            MantalonBody::FormData { form } => {
                let mut debug = f.debug_struct("MantalonBody::FormData");
                form.iter().for_each(|(k, v)| {debug.field(k, v);});
                debug.finish()
            },
            MantalonBody::Known { .. } => f.debug_struct("MantalonBody::Known").finish_non_exhaustive(),
            MantalonBody::ReadableStream { .. } => f.debug_struct("MantalonBody::ReadableStream").finish_non_exhaustive()
        }
    }
}

unsafe impl Send for MantalonBody {}
unsafe impl Sync for MantalonBody {}

/// The errors that can occur when reading from a `MantalonBody`.
#[derive(Debug)]
pub enum MantalonBodyError {
    JsError(String),
}

impl fmt::Display for MantalonBodyError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            MantalonBodyError::JsError(e) => write!(f, "Javascript error: {e}")
        }
    }
}

impl std::error::Error for MantalonBodyError {}

/// Allow conversion from a `ReadableStream` to a `MantalonBody`.
impl From<&ReadableStream> for MantalonBody {
    fn from(stream: &ReadableStream) -> Self {
        let reader = stream.get_reader();
        let reader = reader.dyn_into::<ReadableStreamDefaultReader>().expect("getReader() must return a ReadableStreamDefaultReader");
        MantalonBody::ReadableStream {
            fut: JsFuture::from(reader.read()),
            reader
        }
    }
}

impl Body for MantalonBody {
    type Data = VecDeque<u8>;
    type Error = MantalonBodyError;

    fn poll_frame(
        self: Pin<&mut Self>,
        cx: &mut Context<'_>,
    ) -> Poll<Option<Result<hyper::body::Frame<Self::Data>, Self::Error>>> {
        match self.project() {
            MantalonBodyProj::Empty => Poll::Ready(None),
            MantalonBodyProj::FormData { form } => {
                let data = form.iter().fold(VecDeque::new(), |mut acc, (k, v)| {
                    acc.extend(encode_uri_component(k).as_string().unwrap_or_default().as_bytes());
                    acc.push_back(b'=');
                    acc.extend(encode_uri_component(v).as_string().unwrap_or_default().as_bytes());
                    acc.push_back(b'&');
                    acc
                });
                Poll::Ready(Some(Ok(Frame::data(data))))
            },
            MantalonBodyProj::Known { data } => match data.take() {
                Some(data) => Poll::Ready(Some(Ok(Frame::data(data)))),
                None => Poll::Ready(None),
            },
            MantalonBodyProj::ReadableStream { reader, mut fut } => match fut.as_mut().poll(cx) {
                Poll::Ready(Ok(v)) => {
                    *fut = JsFuture::from(reader.read());
                    let data: Object = v.dyn_into().expect("read() must return an Object");
                    let value = Reflect::get(&data, &"value".into()).expect("value must be present");
                    if value.is_undefined() {
                        return Poll::Ready(None);
                    }
                    let array: Uint8Array = value.dyn_into().expect("value must be a Uint8Array");
                    let buffer: VecDeque<u8> = array.to_vec().into();
                    Poll::Ready(Some(Ok(Frame::data(buffer))))
                },
                Poll::Ready(Err(e)) => Poll::Ready(Some(Err(MantalonBodyError::JsError(format!("{e:?}"))))),
                Poll::Pending => Poll::Pending
            },
        }
    }
    
    fn is_end_stream(&self) -> bool {
        false
    }
    
    fn size_hint(&self) -> hyper::body::SizeHint {
        hyper::body::SizeHint::default()
    }
}
