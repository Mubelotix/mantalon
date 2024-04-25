#![allow(non_snake_case)]

use crate::*;
use http::{HeaderName, HeaderValue, Method};
use wasm_bindgen::prelude::*;
use js_sys::{Array, Function, Map, Reflect::*};

/// Tries to replicate [fetch](https://developer.mozilla.org/en-US/docs/Web/API/fetch)
#[wasm_bindgen]
pub async fn proxiedFetch(ressource: JsValue, options: JsValue) -> Result<JsValue, JsValue> {
    // Read options
    let mut headers = http::HeaderMap::<http::HeaderValue>::new();
    let mut method = Method::GET;
    if let Ok(options) = options.dyn_into::<Map>() {
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
                "headers" => {
                    let Some(value) = value.dyn_into::<Map>().ok() else {
                        error!("Invalid headers");
                        continue
                    };
                    for entry in value.entries().into_iter().filter_map(|e| e.ok()).filter_map(|e| e.dyn_into::<Array>().ok()) {
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
                },
                "integrity" => error!("integrity not supported yet"),
                "keepalive" => error!("keepalive not supported yet"),
                "method" => {
                    let Some(method_str) = value.as_string() else {
                        error!("Invalid method");
                        continue;
                    };
                    match method_str.as_str() {
                        "GET" => method = Method::GET,
                        "POST" => method = Method::POST,
                        "PUT" => method = Method::PUT,
                        "DELETE" => method = Method::DELETE,
                        "HEAD" => method = Method::HEAD,
                        "OPTIONS" => method = Method::OPTIONS,
                        "CONNECT" => method = Method::CONNECT,
                        "PATCH" => method = Method::PATCH,
                        "TRACE" => method = Method::TRACE,
                        unknown => error!("Unknown method: {}", unknown),
                    }
                }
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
    let url = if let Some(url) = ressource.as_string() {
        url
    } else if let Some(url) = get(&ressource, &JsValue::from_str("toString")).ok()
        .and_then(|f| f.dyn_into::<Function>().ok())
        .and_then(|f| apply(&f, &ressource, &Array::new()).ok())
        .and_then(|s| s.as_string())
    {
        url
    } else {
        todo!()
    };

    // Build final request
    let mut request = Request::builder()
        .method(method)
        .uri(url)
        .body(Empty::<Bytes>::new())
        .unwrap();
    *request.headers_mut() = headers;

    // Send request
    match proxied_fetch(request).await {
        Ok(response) => {
            let mut init = ResponseInit::new();
            init.status(response.status().as_u16());
            let headers = Headers::new()?;
            for (name, value) in response.headers() {
                headers.append(name.as_str(), value.to_str().unwrap())?;
            }
            init.headers(&headers);
            let mut body = read_body(response.into_body()).await;

            let js_response = Response::new_with_opt_u8_array_and_init(body.as_deref_mut(), &init)?;

            Ok(js_response.into())
        },
        Err(()) => Err(JsValue::from_str("Error")),
    }
}
