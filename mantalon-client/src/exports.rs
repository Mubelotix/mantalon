#![allow(non_snake_case)]

use crate::*;
use std::{cell::UnsafeCell, rc::Rc};
use http::{header, HeaderName, HeaderValue, Method, Uri};
use js_sys::{Array, ArrayBuffer, Function, Object, Promise, Reflect::{self, *}, Uint8Array};
use web_sys::Request;

fn from_method(value: JsValue) -> Method {
    let Some(method_str) = value.as_string() else {
        error!("Invalid method");
        return Method::GET;
    };
    match method_str.as_str() {
        "GET" => Method::GET,
        "POST" => Method::POST,
        "PUT" => Method::PUT,
        "DELETE" => Method::DELETE,
        "HEAD" => Method::HEAD,
        "OPTIONS" => Method::OPTIONS,
        "CONNECT" => Method::CONNECT,
        "PATCH" => Method::PATCH,
        "TRACE" => Method::TRACE,
        unknown => {
            error!("Unknown method: {}", unknown);
            Method::GET
        },
    }
}

fn from_headers(value: JsValue) -> http::HeaderMap::<http::HeaderValue> {
    let mut headers = http::HeaderMap::<http::HeaderValue>::new();
    let Ok(js_headers) = value.dyn_into::<Headers>() else {
        error!("Invalid headers");
        return headers;
    };
    for entry in js_headers.entries().into_iter().filter_map(|e| e.ok()).filter_map(|e| e.dyn_into::<Array>().ok()) {
        let Some(name): Option<HeaderName> = entry.get(0).as_string().and_then(|n| n.parse().ok()) else {
            error!("Invalid header name");
            continue;
        };
        let Some(value): Option<HeaderValue> = entry.get(1).as_string().and_then(|n| n.parse().ok()) else {
            error!("Invalid header value");
            continue;
        };
        headers.append(name, value);
    }
    headers
}

async fn from_body(value: JsValue) -> Option<(MantalonBody, String)> {
    if value.is_falsy() {
        None
    } else if let Some(string_body) = value.as_string() {
        let body = MantalonBody::Known { data: Some(string_body.into_bytes().into()) };
        let default_content_type = String::from("text/plain");
        Some((body, default_content_type))
    } else if let Some(array_buffer) = value.dyn_ref::<ArrayBuffer>() {
        let array = Uint8Array::new(array_buffer);
        let body = MantalonBody::Known { data: Some(array.to_vec().into()) };
        let default_content_type = String::from("application/octet-stream");
        Some((body, default_content_type))
    } else if let Some(blob) = value.dyn_ref::<web_sys::Blob>() {
        let array_buffer_promise = blob.array_buffer();
        let array_buffer = JsFuture::from(array_buffer_promise).await.expect("arrayBuffer promise must resolve");
        let array = Uint8Array::new(&array_buffer);
        let body = MantalonBody::Known { data: Some(array.to_vec().into()) };
        let default_content_type = blob.type_();
        Some((body, default_content_type))
    } else if let Some(file) = value.dyn_ref::<web_sys::File>() {
        let array_buffer_promise = file.array_buffer();
        let array_buffer = JsFuture::from(array_buffer_promise).await.expect("arrayBuffer promise must resolve");
        let array = Uint8Array::new(&array_buffer);
        let body = MantalonBody::Known { data: Some(array.to_vec().into()) };
        let default_content_type = file.type_();
        Some((body, default_content_type))
    } else if let Some(file) = value.dyn_ref::<web_sys::FormData>() {
        let mut form = Vec::new();
        for entry in file.entries().into_iter().filter_map(|e| e.ok()).filter_map(|e| e.dyn_into::<Array>().ok()) {
            let Some(name): Option<String> = entry.get(0).as_string() else {
                error!("Invalid form field name");
                continue;
            };
            let Some(value): Option<String> = entry.get(1).as_string() else {
                error!("Invalid form field value");
                continue;
            };
            form.push((name, value));
        }
        let body = MantalonBody::FormData { form };
        let default_content_type = String::from("application/x-www-form-urlencoded");
        Some((body, default_content_type))
    } else if let Some(readable_stream) = value.dyn_ref::<ReadableStream>() {
        let body = MantalonBody::from(readable_stream);
        let default_content_type = String::from("application/octet-stream");
        Some((body, default_content_type))
    } else if let Some(_search_params) = value.dyn_ref::<web_sys::UrlSearchParams>() {
        error!("searchParams not supported yet");
        None
    } else {
        error!("Unsupported body type: {value:?}");
        None
    }
}

/// Tries to replicate [fetch](https://developer.mozilla.org/en-US/docs/Web/API/fetch)
/// 
/// Difference: when you set options to a string, it will override the URL.
#[wasm_bindgen]
pub async fn proxiedFetch(ressource: JsValue, options: JsValue) -> Result<JsValue, JsValue> {
    // Read options
    let mut headers = http::HeaderMap::<http::HeaderValue>::new();
    let mut default_content_type = None;
    let mut method = Method::GET;
    let mut url = String::new();
    let mut body = None;
    if let Some(options) = options.as_string() {
        url = options;
    } else if let Ok(options) = options.dyn_into::<Object>() {
        for entry in Object::entries(&options).into_iter().filter_map(|e| e.dyn_into::<Array>().ok()) {
            let Some(key) = entry.get(0).as_string() else {
                error!("Invalid key in options");
                continue;
            };
            let value = entry.get(1);
    
            match key.as_str() {
                "body" => if let Some((b, c)) = from_body(value).await {
                    body = Some(b);
                    default_content_type = Some(c);
                },
                "cache" => error!("cache not supported yet"),
                "credentials" => error!("credentials not supported yet"),
                "headers" => headers = from_headers(value),
                "integrity" => error!("integrity not supported yet"),
                "keepalive" => error!("keepalive not supported yet"),
                "method" => method = from_method(value),
                "mode" => log!("There are no CORS restrictions with Matlon!"),
                "priority" => error!("priority not supported yet"),
                "redirect" => error!("redirect not supported yet"),
                "referrer" => error!("referrer not supported yet"),
                "referrerPolicy" => error!("referrerPolicy not supported yet"),
                unknown => error!("Unknown option: {}", unknown),
            }
        }
    } else {
        error!("Invalid options");
        return Err(JsValue::from_str("Invalid options"));
    }

    // Read ressource
    if let Some(ressource) = ressource.as_string() {
        url = ressource;
    } else {
        match ressource.dyn_into::<Request>() {
            Ok(request) => {
                method = from_method(request.method().into());
                if url.is_empty() {
                    url = request.url();
                }
                headers = from_headers(request.headers().into());
                body = request.body().as_ref().map(|b| b.into());
                if body.is_none() { // Some browsers don't support body
                    if let Ok(array_buffer_promise) = request.array_buffer() {
                        if let Ok(array_buffer) = JsFuture::from(array_buffer_promise).await {
                            if let Ok(array_buffer) = array_buffer.dyn_into::<ArrayBuffer>() {
                                let array = Uint8Array::new(&array_buffer);
                                body = Some(MantalonBody::Known { data: Some(array.to_vec().into()) });
                            }
                        }
                    }
                }
            },
            Err(ressource) => {
                if let Some(ressource) = get(&ressource, &JsValue::from_str("toString")).ok()
                    .and_then(|f| f.dyn_into::<Function>().ok())
                    .and_then(|f| apply(&f, &ressource, &Array::new()).ok())
                    .and_then(|s| s.as_string())
                {
                    url = ressource;
                }
            }
        }
    }

    // Build URI
    let uri = match url.parse::<Uri>() {
        Ok(uri) => uri,
        Err(e) => {
            error!("Invalid URL: {}", e);
            return Err(JsValue::from_str("Invalid URL"));
        },
    };

    // Build request
    let request = http::Request::builder()
        .version(http::Version::HTTP_2)
        .method(method)
        .uri(uri.clone());
    let mut request = match body {
        Some(body) => match request.body(body) {
            Ok(request) => request,
            Err(e) => {
                error!("Error setting body: {e:?}");
                return Err(JsValue::from_str("Error setting body"));
            },
        }
        None => match request.body(MantalonBody::Empty) {
            Ok(request) => request,
            Err(e) => {
                error!("Error setting body: {e:?}");
                return Err(JsValue::from_str("Error setting body"));
            },
        },
    };
    if headers.get(header::CONTENT_TYPE).is_none() {
        if let Some(default_content_type) = default_content_type {
            headers.insert(header::CONTENT_TYPE, default_content_type.parse().unwrap());
        }
    }
    *request.headers_mut() = headers;

    // Send request
    let response = match proxied_fetch(request).await {
        Ok(response) => response,
        Err(error) => {
            error!("Error sending request: {error:?}");
            return Err(JsValue::from_str(&error.to_string()));
        },
    };

    // Start converting response to JS
    let init = ResponseInit::new();
    init.set_status(response.status().as_u16());
    let headers = Headers::new()?;
    let mut counter = 0;
    for (name, value) in response.headers() {
        let value = match value.to_str() {
            Ok(value) => value,
            Err(e) => {
                error!("Error converting header value: {e}");
                continue;
            },
        };
        let name = match name.as_str() {
            "set-cookie" => {
                counter += 1;
                let name = format!("x-mantalon-set-cookie-{counter}");
                headers.set(&name, value)?;
                continue;
            },
            "set-cookie2" => {
                counter += 1;
                let name = format!("x-mantalon-set-cookie2-{counter}");
                headers.set(&name, value)?;
                continue;
            },
            name if name.starts_with("x-mantalon-") => continue, // The server is trying to trick the client
            name => name,
        };
        headers.append(name, value)?;
    }
    init.set_headers(&headers);

    // No body is possible for 204 responses
    if response.status() == 204 {
        return match Response::new_with_opt_str_and_init(None, &init) {
            Ok(js_response) => Ok(js_response.into()),
            Err(e) => {
                error!("Error creating response: {e:?}");
                Err(e)
            },
        }
    }
    
    // Handle the response body
    let body = response.into_body();
    let underlying_source = Object::new();

    let body = Rc::new(UnsafeCell::new(body));
    let pull = Closure::wrap(Box::new(move |controller: ReadableStreamDefaultController| -> Promise {
        let body2 = Rc::clone(&body);
        future_to_promise(async move {
            let body = unsafe { &mut *body2.get() }; // This is safe because the browser will never call pull twice at the same time
            let chunk = body.frame().await;
            match chunk {
                Some(Ok(chunk)) => match chunk.into_data() {
                    Ok(data) => {
                        let data = Uint8Array::from(data.as_ref());
                        controller.enqueue_with_chunk(&data.into())?;
                    },
                    Err(e) => {
                        error!("Received non-data frame: {:?}", e);
                        return Err(JsValue::NULL);
                    }
                },
                Some(Err(err)) => {
                    error!("Error reading chunk: {:?}", err);
                    return Err(JsValue::NULL);
                },
                None => {
                    controller.close()?;
                },
            }
            Ok(JsValue::undefined())
        })
    }) as Box<dyn FnMut(ReadableStreamDefaultController) -> Promise>);
    Reflect::set(&underlying_source, &JsValue::from_str("pull"), pull.as_ref())?;
    let mut pull = Some(pull);

    let cancel = Closure::wrap(Box::new(move |_| {
        pull.take(); // Taking the closure and not doing anything with it will drop it
    }) as Box<dyn FnMut(ReadableStreamDefaultController)>);
    Reflect::set(&underlying_source, &JsValue::from_str("cancel"), cancel.as_ref())?;
    cancel.forget(); // FIXME

    let readable_stream = ReadableStream::new_with_underlying_source(&underlying_source)?;
    match Response::new_with_opt_readable_stream_and_init(Some(&readable_stream), &init) {
        Ok(js_response) => Ok(js_response.into()),
        Err(e) => {
            error!("Error creating streaming response: {e:?}");
            Err(e)
        },
    }
}

#[wasm_bindgen]
pub async fn init(mantalon_endpoint: String) {
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

    MANTALON_ENDPOINT.set(mantalon_endpoint);

    debug!("Mantalon library is ready");
}
