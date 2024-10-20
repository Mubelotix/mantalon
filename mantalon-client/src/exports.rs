#![allow(non_snake_case)]

use std::{cell::UnsafeCell, rc::Rc};
use crate::*;
use http::{HeaderName, HeaderValue, Method, Uri};
use js_sys::{Array, ArrayBuffer, Function, Iterator, Map, Object, Promise, Reflect::{self, *}, Uint8Array};
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
    let entries = get(&value, &JsValue::from_str("entries")).ok()
        .and_then(|f| f.dyn_into::<Function>().ok())
        .and_then(|f| apply(&f, &value, &Array::new()).ok())
        .and_then(|a| a.dyn_into::<Iterator>().ok());
    let entries = match entries {
        Some(entries) => entries,
        None => {
            error!("Invalid headers");
            return headers;
        },
    };
    for entry in entries.into_iter().filter_map(|e| e.ok()).filter_map(|e| e.dyn_into::<Array>().ok()) {
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

/// Tries to replicate [fetch](https://developer.mozilla.org/en-US/docs/Web/API/fetch)
/// 
/// Difference: when you set options to a string, it will override the URL.
#[wasm_bindgen]
pub async fn proxiedFetch(ressource: JsValue, options: JsValue) -> Result<JsValue, JsValue> {
    // Read options
    let mut headers = http::HeaderMap::<http::HeaderValue>::new();
    let mut method = Method::GET;
    let mut url = String::new();
    let mut body = None;
    if let Some(options) = options.as_string() {
        url = options;
    } else if let Ok(options) = options.dyn_into::<Map>() {
        for entry in options.entries().into_iter().filter_map(|e| e.ok()).filter_map(|e| e.dyn_into::<Array>().ok()) {
            let Some(key) = entry.get(0).as_string() else {
                error!("Invalid key in options");
                continue;
            };
            let value = entry.get(1);
    
            match key.as_str() {
                "body" => error!("body not supported yet"),
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
                body = request.body().map(|b| b.into());
                if body.is_none() { // Some shitty browsers don't support body
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
    *request.headers_mut() = headers;

    // Add host header
    if let Some(authority) = uri.authority() {
        if let Ok(host) = authority.host().parse() {
            request.headers_mut().insert("host", host);
        }
    }

    // Send request
    let response = match proxied_fetch(request).await {
        Ok(response) => response,
        Err(error) => {
            error!("Error sending request: {error:?}");
            return Err(JsValue::from_str(&error.to_string()));
        },
    };

    // Start converting response to JS
    let mut init = ResponseInit::new();
    init.status(response.status().as_u16());
    let headers = Headers::new()?;
    for (name, value) in response.headers() {
        let value = match value.to_str() {
            Ok(value) => value,
            Err(e) => {
                error!("Error converting header value: {e}");
                continue;
            },
        };
        headers.append(name.as_str(), value)?;
    }
    init.headers(&headers);
    
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
pub async fn init() {
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

    debug!("Mantalon library is ready");
}
