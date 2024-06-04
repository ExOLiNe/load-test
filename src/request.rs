use std::collections::HashMap;
use std::pin::Pin;
use std::str::Chars;
use strum_macros::Display;
use url::Url;

#[derive(Display)]
pub enum Method {
    GET,
    POST,
    PUT,
    DELETE,
    OPTIONS,
    HEAD
}

pub struct Request<'a> {
    pub method: Method,
    pub url: Url,
    pub headers: HashMap<String, String>,
    pub body: Chars<'a>
}

pub(crate) type ReadyRequest = (Pin<Box<String>>, Pin<Box<String>>);

impl Request<'_> {
    #[async_backtrace::framed]
    pub async fn get_raw(&mut self) -> ReadyRequest {
        let body: Pin<Box<String>> = Pin::new(Box::new(self.body.by_ref().collect()));
        let mut lines = Vec::new();
        println!("{}", self.url.path());
        println!("{}", self.url.authority());
        let query = &self.url.query().map_or_else(|| {
            self.url.path().to_owned()
        }, |q|{
            format!("{}?{}", &self.url.path(), q)
        });
        println!("{}", query);
        lines.push(format!("{} {} HTTP/1.1", &self.method, query));
        lines.push(format!("Host: {}", &self.url.host().unwrap()));
        self.headers.iter().for_each(|(name, value)|{
            lines.push(format!("{}: {}", name, value));
        });
        lines.push(format!("Content-Length: {}", body.len()));
        lines.push(String::from(""));
        lines.push(String::from(""));
        let headers_raw = Pin::new(Box::new(lines.join("\n")));
        println!("{}", headers_raw);
        (headers_raw, body)
    }
}