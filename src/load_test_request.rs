use std::collections::HashMap;
use std::fs;
use std::fs::File;
use std::path::PathBuf;
use std::pin::Pin;
use std::str::FromStr;
use std::sync::Arc;
use lazy_static::lazy_static;

use serde::{Deserialize, Deserializer};
use url::Url;

use crate::error::Error;
use crate::request::{BodyType, Method, Request};

#[cfg(target_os = "linux")]
const DIR_STR: &str = "/mnt/d/RustroverProjects/http_client/";

#[cfg(target_os = "windows")]
const DIR_STR: &str = "D:\\RustroverProjects\\http_client\\";

lazy_static! {
    pub static ref DIRECTORY: PathBuf = PathBuf::from(DIR_STR);
}

#[derive(Deserialize, Debug)]
pub struct LoadTestRequest {
    pub request: RequestData,
    pub repeats: usize,
    pub max_connections: usize
}

#[derive(Deserialize, Debug)]
pub struct RequestData {
    pub query: String,
    pub method: String,
    pub headers: HashMap<String, String>,
    #[serde(deserialize_with = "deserialize_file_to_string")]
    pub body: BodyType
}

impl <'a>TryInto<Request> for &'a RequestData {
    type Error = Error;

    fn try_into(self) -> Result<Request, Self::Error> {
        Ok(Request {
            method: Method::from_str(&self.method)?,
            url: Url::parse(&self.query)?,
            headers: self.headers.clone(),
            body: self.body.clone()
        })
    }
}

fn deserialize_file_to_string<'de, D>(deserializer: D) -> Result<BodyType, D::Error>
where
D: Deserializer<'de>,
{
    Deserialize::deserialize(deserializer).map(|file_path: Option<String>| {
        file_path.map(|file_path| {
            Arc::new(Pin::new(Box::new(fs::read_to_string(format!("{}\\test_data\\body\\{}", DIR_STR, file_path)).unwrap())))
        })
    })
}