use std::collections::HashMap;
use std::pin::Pin;
use std::sync::Arc;
use bytes::Bytes;
use strum_macros::{Display, EnumString};
use url::Url;

use crate::utils::NEWLINE;

#[derive(Display, Debug, PartialEq, EnumString)]
pub enum Method {
    GET,
    POST,
    PUT,
    DELETE,
    OPTIONS,
    HEAD
}

pub(crate) type BodyType = Option<Arc<Pin<String>>>;

pub struct Request {
    pub method: Method,
    pub url: Url,
    pub headers: HashMap<String, String>,
    pub body: BodyType
}

pub(crate) type ReadyRequest = (Pin<Box<Bytes>>, BodyType);

impl Request {
    pub async fn get_raw(&mut self) -> ReadyRequest {
        /*let body: Option<Pin<Box<String>>> = self.body.as_mut().map(|mut body| {
            Pin::new(Box::new(body.by_ref().collect()))
        });*/
        let mut lines = Vec::with_capacity(20);
        let query = &self.url.query().map_or_else(|| {
            self.url.path().to_owned()
        }, |q|{
            format!("{}?{}", &self.url.path(), q)
        });
        lines.push(format!("{} {} HTTP/1.1", &self.method, query));
        lines.push(format!("Host: {}", &self.url.host().expect("Invalid host")));
        self.headers.iter().for_each(|(name, value)|{
            lines.push(format!("{}: {}", name, value));
        });
        let body_len = match &self.body {
            Some(body) => body.len(),
            None => 0
        };
        lines.push(format!("Content-Length: {}", body_len));
        lines.push(String::from(""));
        lines.push(String::from(""));

        let headers_raw = Pin::new(Box::new(Bytes::from(lines.join(NEWLINE))));
        (headers_raw, self.body.clone())
    }
}