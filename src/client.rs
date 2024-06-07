use std::collections::HashMap;
use std::sync::Arc;

use tokio::sync::mpsc::Receiver;
use url::Url;

use crate::connection::{Connection, ConnectionOptions};
use crate::error::{Error};
use crate::header::HttpHeader;
use crate::request::ReadyRequest;

#[derive(Debug)]
pub struct Response {
    pub status: u32,
    pub headers: Vec<HttpHeader>,
    pub body_reader: Option<Receiver<String>>
}

impl Default for Response {
    fn default() -> Self {
        Response {
            status: 0,
            headers: Vec::new(),
            body_reader: None
        }
    }
}

pub struct HttpClient {
    connections: HashMap<String, Connection>
}

impl HttpClient {
    #[async_backtrace::framed]
    pub async fn new() -> HttpClient {
        HttpClient { connections: HashMap::new() }
    }

    #[async_backtrace::framed]
    pub async fn perform_request(
        &mut self,
        url: &Url,
        request: Arc<ReadyRequest>
    ) -> Result<Response, Error> {
        let scheme = url.scheme();
        let host = url.host().expect("There must be domain").to_string();
        let use_tls = scheme == "https";
        let port = url.port().unwrap_or_else(|| {
            if use_tls {
                443u16
            } else {
                80u16
            }
        });

        let url = format!("{}:{}", host, port);
        let url = url.as_str();

        let connection = if let Some(connection) = self.connections.get_mut(url) {
            connection
        } else {
            self.connections.insert(
                url.to_string(),
                Connection::new(host.as_str(), port, use_tls, ConnectionOptions::default()).await?
            );
            self.connections.get_mut(url).expect("Invalid state")
        };

        Ok(connection.send_request(request).await?)
    }
}