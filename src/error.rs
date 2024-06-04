use std::fmt::Debug;

#[derive(Debug)]
pub struct CommonError {
    pub reason: String
}

#[derive(Debug)]
pub enum Error {
    StdError(std::io::Error),
    OwnError(CommonError)
}

/*pub(crate) struct CommonError {
    pub(crate) reason: String
}

impl Debug for CommonError {
    #[async_backtrace::framed]
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.reason.to_string().as_str())
    }
}

impl From<IoError> for CommonError {
    fn from(value: IoError) -> Self {
        CommonError { reason: value.to_string() }
    }
}*/