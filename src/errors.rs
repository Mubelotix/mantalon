use std::str::FromStr;

use actix_web::{http::StatusCode, ResponseError};
use multiaddr::Multiaddr;


#[derive(Debug)]
pub enum MantalonError {
    InvalidAddr { addr: String, error: <Multiaddr as FromStr>::Err },
    MissingProtocol,
    UnsupportedProtocol { protocol: String },
}

impl std::fmt::Display for MantalonError {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            MantalonError::InvalidAddr { addr, error } => write!(f, "Invalid address {addr}: {error}"),
            MantalonError::MissingProtocol => write!(f, "Missing protocol"),
            MantalonError::UnsupportedProtocol { protocol } => write!(f, "Unsupported protocol {protocol}"),
        }
    }
}

impl ResponseError for MantalonError {
    fn status_code(&self) -> StatusCode {
        match self {
            MantalonError::InvalidAddr { .. } => StatusCode::BAD_REQUEST,
            MantalonError::MissingProtocol => StatusCode::BAD_REQUEST,
            MantalonError::UnsupportedProtocol { .. } => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }
}
