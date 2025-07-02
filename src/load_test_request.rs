use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::str::FromStr;
use std::sync::Arc;
use serde::{Deserialize};
use url::Url;

use crate::request::{Method, Request};
use anyhow::Result;

/*#[cfg(target_os = "linux")]
const DIR_STR: &str = "/mnt/d/RustroverProjects/http_client/";

#[cfg(target_os = "windows")]
const DIR_STR: &str = "D:\\RustroverProjects\\http_client\\";*/

/*lazy_static! {
    pub static ref DIRECTORY: PathBuf = PathBuf::from(DIR_STR);
}*/

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
    pub body: Option<PathBuf>
}

pub fn to_request(data: &RequestData, working_dir: &Path) -> Result<Request> {
    /*data.headers.iter().for_each(|(k, v)| {
        debug!("{:?}: {:?}", k.as_bytes(), v.as_bytes());
    });*/
    Ok(Request {
        method: Method::from_str(&data.method)?,
        url: Url::parse(&data.query)?,
        headers: data.headers.clone(),
        body: data.body.clone().map(|body| {
            read_to_body(working_dir.join("body").join(body)).unwrap()
        })
    })
}

pub(crate) fn read_to_body(path: PathBuf) -> Result<Arc<Pin<String>>> {
    Ok(Arc::new(Pin::new(fs::read_to_string(path)?)))
}

/*fn deserialize_file_to_string<'de, D>(deserializer: D) -> Result<BodyType, D::Error>
where
D: Deserializer<'de>,
{
    Deserialize::deserialize(deserializer).map(|file_path: Option<String>| {
        file_path.map(|file_path| {
            Arc::new(Pin::new(Box::new(fs::read_to_string(format!("{}\\test_data\\body\\{}", DIR_STR, file_path)).unwrap())))
        })
    })
}*/