use std::{cell::UnsafeCell, collections::HashMap, ops::Deref};
use crate::*;
use lazy_static::lazy_static;
use serde::{Deserialize, Serialize};
use urlpattern::{UrlPattern, UrlPatternInit};
use wasm_bindgen::{JsCast, JsValue};
use wasm_bindgen_futures::JsFuture;
use web_sys::{window, ServiceWorkerGlobalScope};

#[derive(Debug, Serialize, Deserialize)]
pub struct Manifest {
    pub domains: Vec<String>,
    pub landing_page: String,
    pub https_only: bool,
    pub rewrite_location: bool,
    pub content_edits: Vec<ContentEdit>,    
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ContentEdit {
    pub matches: Vec<String>,
    pub js: Option<String>,
    pub css: Option<String>,
    #[serde(default)]
    pub add_headers: HashMap<String, String>,
    #[serde(default)]
    pub insert_headers: HashMap<String, String>,
}

#[derive(Default, Debug)]
pub struct ParsedManifest {
    pub domains: Vec<String>,
    pub landing_page: String,
    pub https_only: bool,
    pub rewrite_location: bool,
    pub content_edits: Vec<ParsedContentEdit>,
}

#[derive(Debug)]
pub struct ParsedContentEdit {
    pub matches: Vec<UrlPattern>,
    pub js: Option<String>,
    pub css: Option<String>,
    pub add_headers: HashMap<String, String>,
    pub insert_headers: HashMap<String, String>,
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
        }
    }
}

impl std::error::Error for UpdateManifestError {}

pub async fn update_manifest() -> Result<(), UpdateManifestError> {
    let promise = window().map(|w| w.fetch_with_str("/pkg/manifest.json"))
        .or_else(|| js_sys::global().dyn_into::<ServiceWorkerGlobalScope>().ok().map(|sw| sw.fetch_with_str("/pkg/manifest.json")))
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
        https_only: manifest.https_only,
        rewrite_location: manifest.rewrite_location,
        content_edits: Vec::new(),
    };
    for edit in manifest.content_edits {
        let mut matches = Vec::new();
        for pattern in edit.matches {
            let pattern_init = match UrlPatternInit::parse_constructor_string::<regex::Regex>(&pattern, None) {
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
            js: edit.js,
            css: edit.css,
            add_headers: edit.add_headers,
            insert_headers: edit.insert_headers,
        };

        parsed_manifest.content_edits.push(parsed_edit);
    }
    
    let static_manifest: &mut ParsedManifest = unsafe { &mut *MANIFEST.0.get() };
    *static_manifest = parsed_manifest;

    Ok(())
}

#[derive(Default)]
pub struct StaticManifest(UnsafeCell<ParsedManifest>);
unsafe impl Sync for StaticManifest {} // Wasm is singlethreaded
unsafe impl Send for StaticManifest {} // Wasm is singlethreaded

impl Deref for StaticManifest {
    type Target = ParsedManifest;

    fn deref(&self) -> &Self::Target {
        unsafe { &*self.0.get() }
    }
}

lazy_static!{
    pub static ref MANIFEST: StaticManifest = Default::default();
}
