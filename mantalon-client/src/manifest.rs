use std::{cell::UnsafeCell, collections::{HashMap, HashSet}, ops::Deref};
use crate::*;
use http::{uri::{InvalidUri, Parts}, HeaderName, HeaderValue, Uri};
use lazy_static::lazy_static;
use serde::{Deserialize, Serialize};
use url::Url;
use urlpattern::{UrlPattern, UrlPatternInit};
use wasm_bindgen::{JsCast, JsValue};
use wasm_bindgen_futures::JsFuture;
use web_sys::{window, ServiceWorkerGlobalScope};

#[derive(Debug, Serialize, Deserialize)]
pub struct Manifest {
    pub domains: Vec<String>,
    pub lock_browsing: Option<bool>,
    pub https_only: Option<bool>,
    pub rewrite_location: Option<bool>,
    pub js_redirect: Option<bool>,
    pub content_edits: Vec<ContentEdit>,
}

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct ContentEdit {
    pub matches: Vec<String>,
    pub lock_browsing: Option<bool>,
    pub https_only: Option<bool>,
    pub rewrite_location: Option<bool>,
    pub js_redirect: Option<bool>,
    pub js: Option<FileInsertion>,
    pub css: Option<FileInsertion>,
    pub override_url: Option<String>,
    #[serde(default)]
    pub substitute: Vec<Substitution>,
    #[serde(default)]
    pub append_headers: HashMap<String, String>,
    #[serde(default)]
    pub insert_headers: HashMap<String, String>,
    #[serde(default)]
    pub remove_headers: Vec<String>,
    #[serde(default)]
    pub append_request_headers: HashMap<String, String>,
    #[serde(default)]
    pub insert_request_headers: HashMap<String, String>,
    #[serde(default)]
    pub remove_request_headers: Vec<String>,
    #[serde(default)]
    pub rename_headers: HashMap<String, String>,
    #[serde(default)]
    pub rename_request_headers: HashMap<String, String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Substitution {
    pub pattern: String,
    #[serde(default)]
    pub replacement: Option<String>,
    #[serde(default)]
    pub replacement_file: Option<String>,
    #[serde(default)]
    pub max_replacements: Option<usize>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(untagged)]
pub enum FileInsertion {
    File(String),
    Files(Vec<String>),
}

impl From<FileInsertion> for Vec<String> {
    fn from(fi: FileInsertion) -> Vec<String> {
        match fi {
            FileInsertion::File(f) => vec![f],
            FileInsertion::Files(f) => f,
        }
    }
}

#[derive(Debug)]
pub struct ParsedManifest {
    pub domains: Vec<String>,
    pub lock_browsing: bool,
    pub https_only: bool,
    pub rewrite_location: bool,
    pub js_redirect: bool,
    pub content_edits: Vec<ParsedContentEdit>,
}

#[derive(Debug)]
pub struct ParsedContentEdit {
    pub matches: Vec<UrlPattern>,
    pub lock_browsing: bool,
    pub https_only: bool,
    pub rewrite_location: bool,
    pub js_redirect: bool,
    pub js: Vec<String>,
    pub css: Vec<String>,
    pub override_uri: Option<Uri>,
    pub substitute: Vec<Substitution>,
    pub append_headers: HashMap<HeaderName, HeaderValue>,
    pub insert_headers: HashMap<HeaderName, HeaderValue>,
    pub remove_headers: HashSet<HeaderName>,
    pub rename_headers: HashMap<HeaderName, HeaderName>,
    pub append_request_headers: HashMap<HeaderName, HeaderValue>,
    pub insert_request_headers: HashMap<HeaderName, HeaderValue>,
    pub remove_request_headers: HashSet<HeaderName>,
    pub rename_request_headers: HashMap<HeaderName, HeaderName>,
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

async fn load_replacement_file(replacement_file: String) -> Result<String, JsValue> {
    let replacement_url = format!("pkg/config/{replacement_file}");
    let promise = window().map(|w| w.fetch_with_str(&replacement_url))
        .or_else(|| js_sys::global().dyn_into::<ServiceWorkerGlobalScope>().ok().map(|sw| sw.fetch_with_str(&replacement_url)))
        .ok_or(JsValue::from_str("No window or worker location"))?;
    let future = JsFuture::from(promise);
    let response = future.await?;
    let response = response.dyn_into::<web_sys::Response>()?;
    let promise = response.text()?;
    let future = JsFuture::from(promise);
    let text = future.await?;
    let text = text.as_string().expect("response text should be string");
    Ok(text)
}

impl ParsedContentEdit {
    pub fn apply_on_request(&self, request: &mut http::Request<MantalonBody>) {
        if let Some(override_url) = &self.override_uri {
            *request.uri_mut() = override_url.clone(); // FIXME: we might not need to proxy this then
        }
        if self.https_only && request.uri().scheme_str() != Some("https") {
            let mut parts = request.uri().clone().into_parts();
            parts.scheme = Some(http::uri::Scheme::HTTPS);
            *request.uri_mut() = Uri::from_parts(parts).expect("we just changed the scheme")
        }
        
        for header in &self.remove_request_headers {
            request.headers_mut().remove(header);
        }
        for header in &self.insert_request_headers {
            request.headers_mut().insert(header.0.clone(), header.1.clone());
        }
        for header in &self.append_request_headers {
            request.headers_mut().append(header.0.clone(), header.1.clone());
        }

        for header in &self.rename_request_headers {
            if let Some(value) = request.headers_mut().remove(header.0) {
                request.headers_mut().insert(header.1.clone(), value);
            }
        }
    }
    
    pub fn apply_on_response(&self, response: &mut http::Response<Incoming>) {
        // Prevent page from redirecting outside of the proxy
        if self.rewrite_location {
            if let Some(location) = response.headers().get("location").and_then(|l| l.to_str().ok()).and_then(|l| l.parse::<Uri>().ok()) {
                if let Some(authority) = location.authority() {
                    if MANIFEST.domains.iter().any(|d| authority.host() == d) { // TODO: use authority instead of host
                        let mut parts = Parts::default();
                        match SELF_ORIGIN.starts_with("https://") {
                            true => {
                                parts.scheme = Some(http::uri::Scheme::HTTPS);
                                parts.authority = Some(SELF_ORIGIN.trim_start_matches("https://").parse().unwrap());
                            },
                            false => {
                                parts.scheme = Some(http::uri::Scheme::HTTP);
                                parts.authority = Some(SELF_ORIGIN.trim_start_matches("http://").parse().unwrap());
                            }
                        }
                        parts.path_and_query = location.path_and_query().cloned();
                        let uri = Uri::from_parts(parts).unwrap();
                        response.headers_mut().insert("location", uri.to_string().parse().unwrap());
                        response.headers_mut().insert("x-mantalon-location", location.to_string().parse().unwrap());
                    }
                }
            }
        }
    
        for header in &self.remove_headers {
            response.headers_mut().remove(header);
        }
        for header in &self.insert_headers {
            response.headers_mut().insert(header.0.clone(), header.1.clone());
        }
        for header in &self.append_headers {
            response.headers_mut().append(header.0.clone(), header.1.clone());
        }
        for header in &self.rename_headers {
            if let Some(value) = response.headers_mut().remove(header.0) {
                response.headers_mut().insert(header.1.clone(), value);
            }
        }
    }

    pub fn needs_body_response(&self) -> bool {
        self.lock_browsing || !self.js.is_empty() || !self.css.is_empty() || !self.substitute.is_empty() || self.js_redirect
    }

    pub async fn apply_on_response_body(&self, body: &mut Vec<u8>) {
        // TODO: Implement filtering of content edits based on content-type
        // let content_type = response.headers().get("content-type").and_then(|v| v.to_str().ok()).unwrap_or("unknown");
        while body.ends_with(b"\n") {
            body.pop();
        }
        for js_path in &self.js {
            // Only replace when the closing html tag is at the end (easy case)
            if body.ends_with(b"</html>") {
                let len = body.len();
                body.truncate(len - 7);
                body.extend_from_slice(format!("<script src=\"/pkg/config/{js_path}\"></script></html>").as_bytes());
            } else {
                error!("Parse DOM") // TODO: Parse DOM
            }
        }
    
        for css_path in &self.css {
            // Only replace when only one closing head tag is present.
            // Otherwise we don't know which is the right and which might be some XSS attack.
            if let Some(idx) = search_haystack(b"</head>", body) {
                if search_haystack(b"</head>", &body[idx+7..]).is_none() {
                    body.splice(idx..idx+7, format!("<link rel=\"stylesheet\" href=\"/pkg/config/{css_path}\">").into_bytes().into_iter());
                } else {
                    error!("Parse DOM (css)") // TODO: Parse DOM
                }
            }
        }

        if self.lock_browsing {
            // TODO: remove duplicated code
            if body.ends_with(b"</html>") {
                let len = body.len();
                body.truncate(len - 7); 
                let code = include_str!("../scripts/lock_browsing.js");
                let code = code.replace("proxiedDomains", &MANIFEST.domains.iter().map(|d| format!("\"{d}\"")).collect::<Vec<_>>().join(","));
                body.extend_from_slice(format!("<script>{code}</script></html>").as_bytes());
            } else {
                error!("Parse DOM (lock)") // TODO: Parse DOM
            }
        }
        
        for substitution in &self.substitute {
            let pattern = &substitution.pattern;
            let replacement = match &substitution.replacement {
                Some(replacement) => replacement.clone(),
                None => match substitution.replacement_file.clone() {
                    Some(replacement_file) => match load_replacement_file(replacement_file).await {
                        Ok(replacement) => replacement,
                        Err(e) => {
                            error!("Error loading replacement file: {:?}", e);
                            continue;
                        }
                    },
                    None => {
                        error!("Substitution has neither replacement nor replacement_file");
                        continue;
                    }
                }
            };
            let max_replacements = substitution.max_replacements.unwrap_or(usize::MAX);
            replace_in_vec(body, pattern, &replacement, max_replacements);
        }
    }
}

#[derive(Debug)]
#[non_exhaustive]
pub enum UpdateManifestError {
    NoWindow,
    FetchError(JsValue),
    TypeError(JsValue),
    JsonError(JsValue),
    JsonError2(JsValue),
    SerdeError(serde_wasm_bindgen::Error),
    MissingDomain,
    InvalidBaseUrl(url::ParseError),
    InvalidOverrideUrl(InvalidUri),
}

impl std::fmt::Display for UpdateManifestError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            UpdateManifestError::NoWindow => write!(f, "No window available"),
            UpdateManifestError::FetchError(e) => write!(f, "Error fetching manifest: {:?}", e),
            UpdateManifestError::TypeError(e) => write!(f, "Error casting response to Response: {:?}", e),
            UpdateManifestError::JsonError(e) => write!(f, "Error reading body as JSON: {:?}", e),
            UpdateManifestError::JsonError2(e) => write!(f, "Error parsing JSON: {:?}", e),
            UpdateManifestError::SerdeError(e) => write!(f, "Error deserializing manifest: {:?}", e),
            UpdateManifestError::MissingDomain => write!(f, "Manifest must have at least one domain"),
            UpdateManifestError::InvalidBaseUrl(e) => write!(f, "Error parsing base url: {:?}", e),
            UpdateManifestError::InvalidOverrideUrl(e) => write!(f, "Error parsing override url: {:?}", e),
        }
    }
}

impl std::error::Error for UpdateManifestError {}

#[allow(clippy::mutable_key_type)]
fn parse_headers(headers: HashMap<String, String>) -> HashMap<HeaderName, HeaderValue> {
    let mut parsed_headers = HashMap::new();
    for (k, v) in headers {
        let k = match HeaderName::from_bytes(k.as_bytes()) {
            Ok(k) => k,
            Err(e) => {
                error!("Invalid header name {k:?}: {:?}", e);
                continue;
            }
        };
        let v = match HeaderValue::from_bytes(v.as_bytes()) {
            Ok(v) => v,
            Err(e) => {
                error!("Invalid header value {v:?}: {:?}", e);
                continue;
            }
        };
        parsed_headers.insert(k, v);
    }
    parsed_headers
}

#[allow(clippy::mutable_key_type)]
fn parse_header_list(headers: Vec<String>) -> HashSet<HeaderName> {
    let mut parsed_headers = HashSet::new();
    for h in headers {
        let h = match HeaderName::from_bytes(h.as_bytes()) {
            Ok(h) => h,
            Err(e) => {
                error!("Invalid header name {h:?}: {:?}", e);
                continue;
            }
        };
        parsed_headers.insert(h);
    }
    parsed_headers
}

#[allow(clippy::mutable_key_type)]
fn parse_header_rename(headers: HashMap<String, String>) -> HashMap<HeaderName, HeaderName> {
    let mut parsed_headers: HashMap<HeaderName, HeaderName> = HashMap::new();
    for (k, v) in headers {
        let k = match HeaderName::from_bytes(k.as_bytes()) {
            Ok(k) => k,
            Err(e) => {
                error!("Invalid header name {k:?}: {:?}", e);
                continue;
            }
        };
        let v = match HeaderName::from_bytes(v.as_bytes()) {
            Ok(v) => v,
            Err(e) => {
                error!("Invalid header name {v:?}: {:?}", e);
                continue;
            }
        };
        parsed_headers.insert(k, v);
    }
    parsed_headers
}

pub async fn update_manifest(manifest_url: String) -> Result<(), UpdateManifestError> {
    let promise = window().map(|w| w.fetch_with_str(&manifest_url))
        .or_else(|| js_sys::global().dyn_into::<ServiceWorkerGlobalScope>().ok().map(|sw| sw.fetch_with_str(&manifest_url)))
        .ok_or(UpdateManifestError::NoWindow)?;
    let future = JsFuture::from(promise);
    let response = future.await.map_err(UpdateManifestError::FetchError)?;
    let response = response.dyn_into::<web_sys::Response>().map_err(UpdateManifestError::TypeError)?;
    let json_promise = response.json().map_err(UpdateManifestError::JsonError)?;
    let json_future = JsFuture::from(json_promise);
    let json = json_future.await.map_err(UpdateManifestError::JsonError2)?;
    let mut manifest = serde_wasm_bindgen::from_value::<Manifest>(json).map_err(UpdateManifestError::SerdeError)?;
    manifest.content_edits.push(ContentEdit { // Add a wildcard rule to apply root settings to all remaining pages
        matches: vec![String::from("*")],
        ..Default::default()
    });

    // Process some parts of the manifest
    let mut parsed_manifest = ParsedManifest {
        domains: manifest.domains,
        lock_browsing: manifest.lock_browsing.unwrap_or(false),
        https_only: manifest.https_only.unwrap_or(false),
        rewrite_location: manifest.rewrite_location.unwrap_or(true),
        js_redirect: manifest.js_redirect.unwrap_or(false),
        content_edits: Vec::new(),
    };
    let base_domain = parsed_manifest.domains.first().ok_or(UpdateManifestError::MissingDomain)?.clone();
    let base_url = format!("https://{}/", base_domain);
    let base_url = Url::parse(&base_url).map_err(UpdateManifestError::InvalidBaseUrl)?;
    for edit in manifest.content_edits {
        let mut matches = Vec::new();
        for pattern in edit.matches {
            if pattern == "*" {
                let pattern_init = UrlPatternInit::default();
                let pattern = UrlPattern::parse(pattern_init).expect("* is a valid pattern");
                matches.push(pattern);
                continue;
            }
            let pattern_init = match UrlPatternInit::parse_constructor_string::<regex::Regex>(&pattern, Some(base_url.clone())) {
                Ok(init) => init,
                Err(e) => {
                    error!("Invalid pattern {pattern:?}: {:?}", e);
                    continue;
                }
            };
            let pattern = match UrlPattern::parse(pattern_init) {
                Ok(pattern) => pattern,
                Err(e) => {
                    error!("Invalid pattern {pattern:?}: {:?}", e);
                    continue;
                }
            };
            matches.push(pattern);
        }

        let parsed_edit = ParsedContentEdit {
            matches,
            lock_browsing: edit.lock_browsing.unwrap_or(parsed_manifest.lock_browsing),
            https_only: edit.https_only.unwrap_or(parsed_manifest.https_only),
            rewrite_location: edit.rewrite_location.unwrap_or(parsed_manifest.rewrite_location),
            js_redirect: edit.js_redirect.unwrap_or(parsed_manifest.js_redirect),
            js: edit.js.map(FileInsertion::into).unwrap_or_default(),
            css: edit.css.map(FileInsertion::into).unwrap_or_default(),
            override_uri: edit.override_url.map(Uri::try_from).transpose().map_err(UpdateManifestError::InvalidOverrideUrl)?,
            substitute: edit.substitute,
            append_headers: parse_headers(edit.append_headers),
            insert_headers: parse_headers(edit.insert_headers),
            remove_headers: parse_header_list(edit.remove_headers),
            append_request_headers: parse_headers(edit.append_request_headers),
            insert_request_headers: parse_headers(edit.insert_request_headers),
            remove_request_headers: parse_header_list(edit.remove_request_headers),
            rename_headers: parse_header_rename(edit.rename_headers),
            rename_request_headers: parse_header_rename(edit.rename_request_headers),
        };

        parsed_manifest.content_edits.push(parsed_edit);
    }
    debug!("Manifest: {:#?}", parsed_manifest);
    
    let static_manifest: &mut ParsedManifest = unsafe { &mut *MANIFEST.0.get() };
    *static_manifest = parsed_manifest;

    Ok(())
}

pub struct StaticManifest(UnsafeCell<ParsedManifest>);
unsafe impl Sync for StaticManifest {} // Wasm is singlethreaded
unsafe impl Send for StaticManifest {} // Wasm is singlethreaded

impl Default for StaticManifest {
    fn default() -> Self {
        StaticManifest(UnsafeCell::new(ParsedManifest {
            domains: vec!["localhost".to_string()],
            lock_browsing: false,
            https_only: false,
            rewrite_location: true,
            js_redirect: false,
            content_edits: Vec::new(),
        }))
    }
}

impl Deref for StaticManifest {
    type Target = ParsedManifest;

    fn deref(&self) -> &Self::Target {
        unsafe { &*self.0.get() }
    }
}

lazy_static!{
    pub static ref MANIFEST: StaticManifest = Default::default();
}
