use std::rc::Rc;
use cookie::{Cookie, CookieJar};
use http::{Request, Response, Uri};
use tokio::sync::RwLock;
use crate::*;
use lazy_static::lazy_static;

lazy_static!{
    pub static ref GLOBAL_COOKIES: CookieStore = {
        CookieStore {
            cookies: Rc::new(RwLock::new(CookieJar::new())),
            cache_storagage: Rc::new(RwLock::new(None)),
        }
    };
}

#[derive(Default)]
pub struct CookieStore {
    cookies: Rc<RwLock<CookieJar>>,
    cache_storagage: Rc<RwLock<Option<Cache>>>,
}

unsafe impl Send for CookieStore {} // Safe on wasm
unsafe impl Sync for CookieStore {} // Safe on wasm

pub async fn open_cookie_storage() {
    let global = match js_sys::global().dyn_into::<WorkerGlobalScope>() {
        Ok(global) => global,
        Err(_) => {
            error!("No global service worker scope");
            return;
        }
    };
    let caches = match global.caches() {
        Ok(caches) => caches,
        Err(_) => {
            error!("No caches object");
            return;
        }
    };
    let cache_promise = caches.open("mantalon-cookies");
    let cache_fut = JsFuture::from(cache_promise);
    let cache = match cache_fut.await {
        Ok(cache) => cache,
        Err(_) => {
            error!("Error opening cache");
            return;
        }
    };
    let cache = match cache.dyn_into::<Cache>() {
        Ok(cache) => cache,
        Err(_) => {
            error!("Error casting cache");
            return;
        }
    };
    GLOBAL_COOKIES.on_storage_opened(cache).await;
}

#[derive(Debug)]
pub enum CookieError {
    NoOrigin,
    InvalidHeader
}

impl std::fmt::Display for CookieError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CookieError::NoOrigin => write!(f, "No origin in URI"),
            CookieError::InvalidHeader => write!(f, "Invalid cookie header"),
        }
    }
}

impl std::error::Error for CookieError {}

impl CookieStore {
    pub async fn add_cookies(&self, request: &mut Request<MantalonBody>) -> Result<(), CookieError> {
        let uri = request.uri();
        let request_domain = uri.host().ok_or(CookieError::NoOrigin)?; // TODO: Are ports ignored by cookies?
        let cookies = self.cookies.read().await;

        let mut cookie_list = Vec::new(); // TODO: Optimize using string
        for cookie in cookies.iter() { // TODO: include sameSite cookies
            if let Some(domain) = cookie.domain() { // TODO: Handle cookies without a domain
                if !request_domain.ends_with(domain) {
                    continue;
                }
            }
            if let Some(path) = cookie.path() {
                if !uri.path().starts_with(path) {
                    continue;
                }
            }
            let cookie = cookie.stripped().encoded().to_string();
            cookie_list.push(cookie);
        }
        let cookie_header = cookie_list.join("; ");
        let cookie_header = cookie_header.parse().map_err(|_| CookieError::InvalidHeader)?;
        request.headers_mut().insert("cookie", cookie_header); // TODO: is insert the best choice?

        Ok(())
    }

    pub async fn store_cookies<B>(&self, uri: &Uri, response: &Response<B>) -> Result<(), CookieError> {
        let request_domain = uri.host().ok_or(CookieError::NoOrigin)?;
        let mut cookies = self.cookies.write().await;

        let mut changed = false;
        for cookie_header in response.headers().get_all("set-cookie") {
            let cookie_header = match cookie_header.to_str() {
                Ok(s) => s,
                Err(e) => {
                    error!("Invalid cookie header: {e}");
                    continue;
                }
            };
            let cookie = match Cookie::parse_encoded(cookie_header) {
                Ok(cookie) => cookie,
                Err(e) => {
                    error!("Error parsing cookie in {cookie_header:?}: {e}");
                    continue;
                },
            };
            if let Some(domain) = cookie.domain() {
                if !request_domain.ends_with(domain) {
                    error!("Cookie domain {domain} does not match origin {request_domain}");
                    continue;
                }
            }
            cookies.add(cookie.into_owned());
            changed = true;
        }

        if changed {
            spawn_local(async move { // TODO: using global not logical
                GLOBAL_COOKIES.save().await;
            });
        }

        Ok(())
    }

    pub async fn on_storage_opened(&self, cache: Cache) {
        self.cache_storagage.write().await.replace(cache.clone());

        let match_promise = cache.match_with_str("cookies");
        let match_fut = JsFuture::from(match_promise);
        let match_result = match match_fut.await {
            Ok(match_result) => match_result,
            Err(_) => {
                error!("Error matching cache");
                return;
            }
        };
        let match_response = match match_result.dyn_into::<web_sys::Response>() {
            Ok(match_response) => match_response,
            Err(_) => {
                error!("Error casting match response");
                return;
            }
        };
        let match_text_promise = match match_response.text() {
            Ok(match_text_promise) => match_text_promise,
            Err(_) => {
                error!("Error getting text promise from match response");
                return;
            }
        };
        let match_text_fut = JsFuture::from(match_text_promise);
        let match_text = match match_text_fut.await {
            Ok(match_text) => match_text,
            Err(_) => {
                error!("Error getting text from match response");
                return;
            }
        };
        let match_text = match match_text.as_string() {
            Some(match_text) => match_text,
            None => {
                error!("Error getting string from match response");
                return;
            }
        };
        let mut cookies = self.cookies.write().await;
        for cookie in match_text.lines() {
            let cookie = match Cookie::parse_encoded(cookie) {
                Ok(cookie) => cookie,
                Err(e) => {
                    error!("Error parsing cookie: {e}");
                    continue;
                }
            };
            cookies.add(cookie.into_owned());
        }
    }

    pub async fn save(&self) {
        let cookies = self.cookies.read().await;
        let data = cookies.iter().map(|c| c.encoded().to_string()).collect::<Vec<String>>().join("\n");
        drop(cookies);

        let cache = self.cache_storagage.read().await;
        let cache = match cache.as_ref() {
            Some(cache) => cache,
            None => {
                error!("No cache to save cookies to");
                return;
            }
        };

        let resp = match web_sys::Response::new_with_opt_str(Some(data.as_str())) {
            Ok(resp) => resp,
            Err(_) => {
                error!("Error creating response");
                return;
            }
        };
        let put_promise = cache.put_with_str("cookies", &resp);
        let put_fut = JsFuture::from(put_promise);
        match put_fut.await {
            Ok(_) => (),
            Err(_) => {
                error!("Error putting cookies in cache");
            }
        }
    }
}

pub trait HackTraitOrigin { // TODO: move to more appropriate location
    fn origin(&self) -> Option<String>;
}

impl HackTraitOrigin for Uri {
    fn origin(&self) -> Option<String> {
        let scheme = self.scheme_str()?;
        let host = self.host()?;
        match self.port_u16() {
            Some(port) => Some(format!("{scheme}://{host}:{port}")),
            None => Some(format!("{scheme}://{host}"))
        }
    }
}
