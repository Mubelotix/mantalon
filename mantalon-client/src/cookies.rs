use std::{collections::HashMap, rc::Rc};
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
    cookies: Rc<RwLock<HashMap<String, CookieJar>>>,
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
        let origin = uri.origin().ok_or(CookieError::NoOrigin)?;
        let all_cookies = self.cookies.read().await;
        let cookies = match all_cookies.get(&origin) {
            Some(jar) => jar,
            None => return Ok(()),
        };

        let mut cookie_list = Vec::new(); // TODO: Optimize using string
        for cookie in cookies.iter() {
            let cookie = cookie.stripped().encoded().to_string();
            cookie_list.push(cookie);
        }
        let cookie_header = cookie_list.join("; ");
        let cookie_header = cookie_header.parse().map_err(|_| CookieError::InvalidHeader)?;
        request.headers_mut().insert("cookie", cookie_header); // TODO: is insert the best choice?

        Ok(())
    }

    pub async fn store_cookies<B>(&self, uri: &Uri, response: &Response<B>) -> Result<(), CookieError> {
        let origin = uri.origin().ok_or(CookieError::NoOrigin)?;

        let mut all_cookies = self.cookies.write().await;
        let cookies = all_cookies.entry(origin).or_insert_with(CookieJar::new);

        for cookie_header in response.headers().get_all("set-cookie") {
            let cookie_header = match cookie_header.to_str() {
                Ok(s) => s,
                Err(_) => {
                    error!("Invalid cookie header");
                    continue;
                }
            };
            let new_cookies = Cookie::split_parse_encoded(cookie_header);
            for cookie in new_cookies {
                match cookie {
                    Ok(cookie) => cookies.add(cookie.into_owned()),
                    Err(e) => {
                        error!("Error parsing cookie: {e}");
                        continue;
                    },
                }
            }
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
