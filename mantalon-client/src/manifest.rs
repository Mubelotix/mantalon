use std::{cell::UnsafeCell, collections::HashMap, ops::Deref};
use lazy_static::lazy_static;
use serde::{Deserialize, Serialize};
use wasm_bindgen::{JsCast, JsValue};
use wasm_bindgen_futures::JsFuture;
use web_sys::{window, ServiceWorkerGlobalScope};


#[derive(Default, Debug, Serialize, Deserialize)]
pub struct MantalonManifest {
    domains: Vec<String>,
    landing_page: String,
    https_only: bool,
    rewrite_location: bool,
    content_edits: Vec<ContentEdit>,    
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ContentEdit {
    matches: Vec<String>,
    js: Option<String>,
    css: Option<String>,
    #[serde(default)]
    add_headers: HashMap<String, String>,
    #[serde(default)]
    insert_headers: HashMap<String, String>,
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
    let manifest = serde_wasm_bindgen::from_value::<MantalonManifest>(json).map_err(UpdateManifestError::SerdeError)?;
    
    let static_manifest: &mut MantalonManifest = unsafe { &mut *MANIFEST.0.get() };
    *static_manifest = manifest;

    Ok(())
}

#[derive(Default)]
pub struct StaticManifest(UnsafeCell<MantalonManifest>);
unsafe impl Sync for StaticManifest {} // Wasm is singlethreaded
unsafe impl Send for StaticManifest {} // Wasm is singlethreaded

impl Deref for StaticManifest {
    type Target = MantalonManifest;

    fn deref(&self) -> &Self::Target {
        unsafe { &*self.0.get() }
    }
}

lazy_static!{
    pub static ref MANIFEST: StaticManifest = Default::default();
}