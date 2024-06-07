use std::collections::HashMap;
use std::fs;
use std::pin::Pin;
use std::rc::Rc;
use std::str::FromStr;
use std::sync::Arc;
use serde::{Deserialize, Deserializer};
use url::Url;
use crate::error::Error;
use crate::request::{BodyType, Method, Request};

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
            Arc::new(Pin::new(Box::new(fs::read_to_string(format!("D:\\RustroverProjects\\http_client\\test_data\\body\\{}", file_path)).unwrap())))
        })
    })
    /*let file_path = String::deserialize(deserializer)?;
    fs::read_to_string(format!("D:\\RustroverProjects\\http_client\\test_data\\body\\{}", &file_path)).map_err(serde::de::Error::custom)*/
}