#![allow(non_snake_case)]

use crate::*;
use http::{HeaderName, HeaderValue, Method, Uri};
use js_sys::{Array, Function, Iterator, Map, Reflect::*};
use url::Url;
use urlpattern::UrlPatternMatchInput;
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

pub async fn complete_response(response: http::Response<Incoming>) -> http::Response<Vec<u8>> {
    let (head, body) = response.into_parts();
    let body = read_body(body).await;
    http::Response::from_parts(head, body.unwrap_or_default())
}

fn search_haystack<T: PartialEq>(needle: &[T], haystack: &[T]) -> Option<usize> {
    if needle.is_empty() {
        return Some(0);
    }

    haystack.windows(needle.len()).position(|subslice| subslice == needle)
}

pub fn get_content_edit(request: &http::Request<Empty<Bytes>>) -> Option<&'static ParsedContentEdit> {
    let url = Url::parse(&request.uri().to_string()).unwrap();
    let pattern_match_input = UrlPatternMatchInput::Url(url);
    MANIFEST.content_edits.iter().find(|&ce| ce.matches.iter().any(|pattern| pattern.test(pattern_match_input.clone()).unwrap_or_default()))
}

pub async fn apply_edit_request(request: &mut http::Request<Empty<Bytes>>, content_edit: &ParsedContentEdit) {
    if let Some(override_url) = &content_edit.override_uri {
        *request.uri_mut() = override_url.clone(); // FIXME: we might not need to proxy this then
    }
}

pub async fn apply_edit_response(response: &mut http::Response<Vec<u8>>, content_edit: &ParsedContentEdit) {
    let content_type = response.headers().get("content-type").and_then(|v| v.to_str().ok()).unwrap_or("unknown");
    if content_type == "text/html" || content_type.starts_with("text/html;") {
        if let Some(js_path) = &content_edit.js {
            // Only replace when the closing html tag is at the end (easy case)
            if response.body().ends_with(b"</html>") {
                let len = response.body().len();
                response.body_mut().truncate(len - 7);
                response.body_mut().extend_from_slice(format!("<script src=\"/pkg/config/{js_path}\"></script></html>").as_bytes());
            } else {
                error!("Parse DOM") // TODO: Parse DOM
            }
        }
    
        if let Some(css_path) = &content_edit.css {
            // Only replace when only one closing head tag is present.
            // Otherwise we don't know which is the right and which might be some XSS attack.
            if let Some(idx) = search_haystack(b"</head>", response.body()) {
                if search_haystack(b"</head>", &response.body()[idx+7..]).is_none() {
                    response.body_mut().splice(idx..idx+7, format!("<link rel=\"stylesheet\" href=\"/pkg/config/{css_path}\">").into_bytes().into_iter());
                } else {
                    error!("Parse DOM (css)") // TODO: Parse DOM
                }
            }
        }
    }
    
    for substitution in &content_edit.substitute {
        let pattern = &substitution.pattern;
        let replacement = &substitution.replacement;
        let max_replacements = substitution.max_replacements.unwrap_or(usize::MAX);
        let mut idx_start = 0;
        let mut i = 0;
        while let Some(idx) = search_haystack(pattern.as_bytes(), &response.body()[idx_start..]) {
            response.body_mut().splice(idx..idx+pattern.len(), replacement.clone().into_bytes().into_iter());
            idx_start += idx + replacement.len();
            i += 1;
            if i >= max_replacements {
                break;
            }
        }
    }
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

    // Build URI
    let uri = match url.parse::<Uri>() {
        Ok(uri) => {
            let mut parts = uri.into_parts();
            if let Some(authority) = &mut parts.authority {
                if authority.host() == "127.0.0.1" || authority.host() == "localhost" {
                    *authority = MANIFEST.base_authority.clone();
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

    // Build request
    let mut request = http::Request::builder()
        .method(method)
        .uri(uri.clone())
        .body(Empty::<Bytes>::new())
        .unwrap();
    *request.headers_mut() = headers;

    // Get content edit
    let mut content_edit = get_content_edit(&request);

    // Apply edit on request
    if let Some(ce) = content_edit {
        apply_edit_request(&mut request, ce).await;
        content_edit = get_content_edit(&request);
    }

    // Send request
    let mut response = match proxied_fetch(request).await {
        Ok(response) => complete_response(response).await,
        Err(()) => return Err(JsValue::from_str("Error")),
    };

    // Apply edit on response
    if let Some(ce) = content_edit {
        apply_edit_response(&mut response, ce).await;
    }

    // Prevent page from redirecting outside of the proxy
    if let Some(location) = response.headers().get("location").and_then(|l| l.to_str().ok()).and_then(|l| l.parse::<Uri>().ok()) {
        let mut parts = location.into_parts();
        if let Some(mut authority) = parts.authority {
            if MANIFEST.domains.iter().any(|d| authority.host() == d) { // TODO: use authority instead of host
                parts.scheme = Some(http::uri::Scheme::HTTP);
                authority = "localhost:8000".parse().unwrap(); // TODO: Unhardcode
                parts.authority = Some(authority);
                let uri = Uri::from_parts(parts).unwrap();
                response.headers_mut().insert("location", uri.to_string().parse().unwrap());
            }
        }
    }

    // Convert response to JS
    let mut init = ResponseInit::new();
    init.status(response.status().as_u16());
    let headers = Headers::new()?;
    for (name, value) in response.headers() {
        headers.append(name.as_str(), value.to_str().unwrap())?;
    }
    init.headers(&headers);
    let mut body = response.into_body();
    let js_response = Response::new_with_opt_u8_array_and_init(Some(&mut body), &init)?;

    Ok(js_response.into())
}

#[wasm_bindgen]
pub fn getProxiedDomains() -> Array {
    Array::from_iter(MANIFEST.domains.iter().map(|d| JsValue::from_str(d)))
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

    update_manifest().await.expect("Error updating manifest");

    debug!("Proxy library ready. Proxying {}", MANIFEST.domains.join(", "));
}
