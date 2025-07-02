use strum_macros::Display;

#[derive(Debug, Display)]
pub enum MyError {
    ConnectionClosedUnexpectedly,
    UnknownError,
    ZeroRead,
    HeaderParseError,
    IpResolve,
    IdleTimeout
}

impl std::error::Error for MyError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            MyError::ConnectionClosedUnexpectedly => None,
            MyError::UnknownError => None,
            MyError::ZeroRead => None,
            MyError::HeaderParseError => None,
            MyError::IpResolve => None,
            MyError::IdleTimeout => None
        }
    }
}