#![allow(non_snake_case)]

use crate::*;
use http::{HeaderName, HeaderValue, Method, Uri};
use js_sys::{Array, Function, Iterator, Map, Reflect::*};
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
#[wasm_bindgen]
pub async fn proxiedFetch(ressource: JsValue, options: JsValue) -> Result<JsValue, JsValue> {
    // Read options
    let mut headers = http::HeaderMap::<http::HeaderValue>::new();
    let mut method = Method::GET;
    let mut url = String::new();
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
                url = request.url();
                headers = from_headers(request.headers().into());
                // TODO body
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

    // Build final request
    let uri = match url.parse::<Uri>() {
        Ok(uri) => {
            let mut parts = uri.into_parts();
            if let Some(authority) = &mut parts.authority {
                if authority.host() == "127.0.0.1" {
                    *authority = "en.wikipedia.org".parse().unwrap();
                    parts.scheme = Some("https".parse().unwrap());
                }
            }
            Uri::from_parts(parts).unwrap()
        },
        Err(e) => {
            error!("Invalid URL: {}", e);
            return Err(JsValue::from_str("Invalid URL"));
        },
    };
    let mut request = http::Request::builder()
        .method(method)
        .uri(uri)
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
