use std::{cell::UnsafeCell, collections::HashMap, ops::Deref};
use crate::*;
use http::{uri::InvalidUri, HeaderName, HeaderValue, Uri};
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
    pub landing_page: String,
    pub lock_browsing: Option<bool>,
    pub https_only: bool,
    pub rewrite_location: bool,
    pub content_edits: Vec<ContentEdit>,    
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ContentEdit {
    pub matches: Vec<String>,
    pub lock_browsing: Option<bool>,
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
    pub append_request_headers: HashMap<String, String>,
    #[serde(default)]
    pub insert_request_headers: HashMap<String, String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Substitution {
    pub pattern: String,
    pub replacement: String,
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
    pub landing_page: String,
    pub lock_browsing: bool,
    pub https_only: bool,
    pub rewrite_location: bool,
    pub content_edits: Vec<ParsedContentEdit>,
}

#[derive(Debug)]
pub struct ParsedContentEdit {
    pub matches: Vec<UrlPattern>,
    pub lock_browsing: bool,
    pub js: Vec<String>,
    pub css: Vec<String>,
    pub override_uri: Option<Uri>,
    pub substitute: Vec<Substitution>,
    pub append_headers: HashMap<HeaderName, HeaderValue>,
    pub insert_headers: HashMap<HeaderName, HeaderValue>,
    pub append_request_headers: HashMap<HeaderName, HeaderValue>,
    pub insert_request_headers: HashMap<HeaderName, HeaderValue>,
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

pub async fn update_manifest() -> Result<(), UpdateManifestError> {
    let promise = window().map(|w| w.fetch_with_str("/pkg/config/manifest.json"))
        .or_else(|| js_sys::global().dyn_into::<ServiceWorkerGlobalScope>().ok().map(|sw| sw.fetch_with_str("/pkg/config/manifest.json")))
        .ok_or(UpdateManifestError::NoWindow)?;
    let future = JsFuture::from(promise);
    let response = future.await.map_err(UpdateManifestError::FetchError)?;
    let response = response.dyn_into::<web_sys::Response>().map_err(UpdateManifestError::TypeError)?;
    let json_promise = response.json().map_err(UpdateManifestError::JsonError)?;
    let json_future = JsFuture::from(json_promise);
    let json = json_future.await.map_err(UpdateManifestError::JsonError2)?;
    let manifest = serde_wasm_bindgen::from_value::<Manifest>(json).map_err(UpdateManifestError::SerdeError)?;

    // Process some parts of the manifest
    let mut parsed_manifest = ParsedManifest {
        domains: manifest.domains,
        landing_page: manifest.landing_page,
        lock_browsing: manifest.lock_browsing.unwrap_or(false),
        https_only: manifest.https_only,
        rewrite_location: manifest.rewrite_location,
        content_edits: Vec::new(),
    };
    let base_domain = parsed_manifest.domains.first().ok_or(UpdateManifestError::MissingDomain)?.clone();
    let base_url = format!("https://{}/", base_domain);
    let base_url = Url::parse(&base_url).map_err(UpdateManifestError::InvalidBaseUrl)?;
    for edit in manifest.content_edits {
        let mut matches = Vec::new();
        for pattern in edit.matches {
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
            js: edit.js.map(FileInsertion::into).unwrap_or_default(),
            css: edit.css.map(FileInsertion::into).unwrap_or_default(),
            override_uri: edit.override_url.map(Uri::try_from).transpose().map_err(UpdateManifestError::InvalidOverrideUrl)?,
            substitute: edit.substitute,
            append_headers: parse_headers(edit.append_headers),
            insert_headers: parse_headers(edit.insert_headers),
            append_request_headers: parse_headers(edit.append_request_headers),
            insert_request_headers: parse_headers(edit.insert_request_headers),
        };

        parsed_manifest.content_edits.push(parsed_edit);
    }
    
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
            landing_page: "/".to_string(),
            https_only: false,
            rewrite_location: false,
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
