use std::fmt::{Debug, Display, Formatter};
use std::net::AddrParseError;
use std::num::ParseIntError;
use std::time::SystemTimeError;
use bytes::Bytes;
use strum::ParseError;

pub(crate) fn _is_normal<T: Sized + Send + Sync + Unpin>() {}

#[derive(Debug)]
pub struct CommonError {
    pub reason: String
}

#[derive(Debug)]
pub enum Error {
    IO(std::io::Error),
    Crate(CommonError),
    HeaderParse(&'static str),
    ParseInt(ParseIntError),
    AddrParse(AddrParseError),
    StrumParse(ParseError),
    UrlParse(url::ParseError),
    MpscSend(tokio::sync::mpsc::error::SendError<Bytes>),
    Serde(serde_yaml::Error),
    SystemTime(SystemTimeError),
    NativeTls(native_tls::Error),
    IpResolve(String),
    IdleTimeout,
    ZeroRead,
    ParseStatus,
    ConnectionClosedUnexpectedly
}

unsafe impl Send for Error {}

impl Display for Error {
    fn fmt(&self, _: &mut Formatter<'_>) -> std::fmt::Result {
        todo!("IMPLEMENT THIS FOOKIN EROOOO")
    }
}

impl std::error::Error for Error {
}

impl From<std::io::Error> for Error {
    fn from(value: std::io::Error) -> Self {
        Self::IO(value)
    }
}

impl From<ParseIntError> for Error {
    fn from(value: ParseIntError) -> Self {
        Self::ParseInt(value)
    }
}

impl From<AddrParseError> for Error {
    fn from(value: AddrParseError) -> Self {
        Error::AddrParse(value)
    }
}

impl From<ParseError> for Error {
    fn from(value: ParseError) -> Self {
        Error::StrumParse(value)
    }
}

impl From<url::ParseError> for Error {
    fn from(value: url::ParseError) -> Self {
        Error::UrlParse(value)
    }
}

impl From<tokio::sync::mpsc::error::SendError<Bytes>> for Error {
    fn from(value: tokio::sync::mpsc::error::SendError<Bytes>) -> Self {
        Error::MpscSend(value)
    }
}

impl From<serde_yaml::Error> for Error {
    fn from(value: serde_yaml::Error) -> Self {
        Error::Serde(value)
    }
}

impl From<SystemTimeError> for Error {
    fn from(value: SystemTimeError) -> Self {
        Error::SystemTime(value)
    }
}

impl From<native_tls::Error> for Error {
    fn from(value: native_tls::Error) -> Self {
        Error::NativeTls(value)
    }
}
