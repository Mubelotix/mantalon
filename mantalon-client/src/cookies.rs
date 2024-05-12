use std::rc::Rc;
use cookie::{Cookie, CookieJar};
use http::{Request, Response, Uri};
use tokio::sync::RwLock;
use crate::*;
use lazy_static::lazy_static;

lazy_static!{
    pub static ref GLOBAL_COOKIES: CookieStore = CookieStore::default(); // TODO restore cookies from storage
}

#[derive(Default)]
pub struct CookieStore {
    cookies: Rc<RwLock<CookieJar>>,
}

unsafe impl Send for CookieStore {} // Safe on wasm
unsafe impl Sync for CookieStore {} // Safe on wasm

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
            cookies.add(cookie.into_owned())
        }

        Ok(())
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
