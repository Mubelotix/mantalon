#![allow(non_snake_case)]

use crate::*;
use http::{uri::Parts, HeaderName, HeaderValue, Method, Uri};
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

fn replace_in_vec(data: &mut Vec<u8>, pattern: &String, replacement: &String, max_replacements: usize) {
    let mut idx_start = 0;
    let mut i = 0;
    while let Some(idx) = search_haystack(pattern.as_bytes(), &data[idx_start..]) {
        data.splice(idx_start+idx..idx_start+idx+pattern.len(), replacement.as_bytes().iter().copied());
        idx_start += idx + replacement.len();
        i += 1;
        if i >= max_replacements {
            break;
        }
    }
}

pub fn get_content_edit(request: &http::Request<Empty<Bytes>>) -> (usize, &'static ParsedContentEdit) {
    let url = Url::parse(&request.uri().to_string()).unwrap();
    let pattern_match_input = UrlPatternMatchInput::Url(url);
    MANIFEST.content_edits
        .iter()
        .enumerate()
        .find(|(_, ref ce)| ce.matches.iter().any(|pattern| pattern.test(pattern_match_input.clone())
        .unwrap_or_default()))
        .unwrap() // There is always a wildcard match
}

pub async fn apply_edit_request(request: &mut http::Request<Empty<Bytes>>, content_edit: &ParsedContentEdit) {
    if let Some(override_url) = &content_edit.override_uri {
        *request.uri_mut() = override_url.clone(); // FIXME: we might not need to proxy this then
    }
    if content_edit.https_only && request.uri().scheme_str() != Some("https") {
        let mut parts = request.uri().clone().into_parts();
        parts.scheme = Some(http::uri::Scheme::HTTPS);
        *request.uri_mut() = Uri::from_parts(parts).unwrap();
    }
    for header in &content_edit.remove_request_headers {
        request.headers_mut().remove(header);
    }
    for header in &content_edit.insert_request_headers {
        request.headers_mut().insert(header.0.clone(), header.1.clone());
    }
    for header in &content_edit.append_request_headers {
        request.headers_mut().append(header.0.clone(), header.1.clone());
    }
}

pub async fn apply_edit_response(response: &mut http::Response<Vec<u8>>, content_edit: &ParsedContentEdit) {
    // Prevent page from redirecting outside of the proxy
    if content_edit.rewrite_location {
        if let Some(location) = response.headers().get("location").and_then(|l| l.to_str().ok()).and_then(|l| l.parse::<Uri>().ok()) {
            if let Some(authority) = location.authority() {
                if MANIFEST.domains.iter().any(|d| authority.host() == d) { // TODO: use authority instead of host
                    let mut parts = Parts::default();
                    parts.scheme = Some(http::uri::Scheme::HTTP);
                    parts.authority = Some("localhost:8000".parse().unwrap()); // TODO: Unhardcode
                    parts.path_and_query = location.path_and_query().cloned();
                    let uri = Uri::from_parts(parts).unwrap();
                    response.headers_mut().insert("location", uri.to_string().parse().unwrap());
                    response.headers_mut().insert("x-mantalon-location", location.to_string().parse().unwrap());
                }
            }
        }
    }

    let content_type = response.headers().get("content-type").and_then(|v| v.to_str().ok()).unwrap_or("unknown");
    if content_type == "text/html" || content_type.starts_with("text/html;") {
        for js_path in &content_edit.js {
            // Only replace when the closing html tag is at the end (easy case)
            if response.body().ends_with(b"</html>") {
                let len = response.body().len();
                response.body_mut().truncate(len - 7);
                response.body_mut().extend_from_slice(format!("<script src=\"/pkg/config/{js_path}\"></script></html>").as_bytes());
            } else {
                error!("Parse DOM") // TODO: Parse DOM
            }
        }
    
        for css_path in &content_edit.css {
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

        if content_edit.lock_browsing {
            // TODO: remove duplicated code
            if response.body().ends_with(b"</html>") {
                let len = response.body().len();
                response.body_mut().truncate(len - 7); 
                let code = include_str!("../scripts/lock_browsing.js");
                let code = code.replace("proxiedDomains", &MANIFEST.domains.iter().map(|d| format!("\"{d}\"")).collect::<Vec<_>>().join(","));
                response.body_mut().extend_from_slice(format!("<script>{code}</script></html>").as_bytes());
            } else if response.body().ends_with(b"</html>\n") {
                let len = response.body().len();
                response.body_mut().truncate(len - 8); 
                let code = include_str!("../scripts/lock_browsing.js");
                let code = code.replace("proxiedDomains", &MANIFEST.domains.iter().map(|d| format!("\"{d}\"")).collect::<Vec<_>>().join(","));
                response.body_mut().extend_from_slice(format!("<script>{code}</script></html>\n").as_bytes());
            } else {
                error!("Parse DOM (lock)") // TODO: Parse DOM
            }
        }
    }
    
    for substitution in &content_edit.substitute {
        let pattern = &substitution.pattern;
        let replacement = match &substitution.replacement {
            Some(replacement) => replacement.clone(),
            None => {
                let replacement_file = substitution.replacement_file.clone().unwrap(); // TODO ensure existence
                let replacement_url = format!("pkg/config/{replacement_file}");
                let promise = window().map(|w| w.fetch_with_str(&replacement_url))
                    .or_else(|| js_sys::global().dyn_into::<ServiceWorkerGlobalScope>().ok().map(|sw| sw.fetch_with_str(&replacement_url)))
                    .unwrap();
                let future = JsFuture::from(promise);
                let response = future.await.unwrap();
                let response = response.dyn_into::<web_sys::Response>().unwrap();
                let promise = response.text().unwrap();
                let future = JsFuture::from(promise);
                let text = future.await.unwrap();
                text.as_string().unwrap()
            }
        };
        let max_replacements = substitution.max_replacements.unwrap_or(usize::MAX);
        replace_in_vec(response.body_mut(), pattern, &replacement, max_replacements);
    }

    for header in &content_edit.remove_headers {
        response.headers_mut().remove(header);
    }
    for header in &content_edit.insert_headers {
        response.headers_mut().insert(header.0.clone(), header.1.clone());
    }
    for header in &content_edit.append_headers {
        response.headers_mut().append(header.0.clone(), header.1.clone());
    }
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
        Ok(uri) => uri,
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
    let (i, mut content_edit) = get_content_edit(&request); // There is always a wildcard match
    debug!("Using content edit {i} for {uri}");

    // Apply edit on request
    apply_edit_request(&mut request, content_edit).await;
    content_edit = get_content_edit(&request).1;

    // Add host header
    if let Some(authority) = uri.authority() {
        if let Ok(host) = authority.host().parse() {
            request.headers_mut().insert("host", host);
        }
    }

    // Send request
    let mut response = match proxied_fetch(request).await {
        Ok(response) => complete_response(response).await,
        Err(error) => return Err(JsValue::from_str(&error.to_string())),
    };

    // Apply edit on response
    apply_edit_response(&mut response, content_edit).await;

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
