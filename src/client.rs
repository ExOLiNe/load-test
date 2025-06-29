use std::collections::HashMap;
use std::sync::Arc;
use bytes::Bytes;
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
    pub body_reader: Option<Receiver<Bytes>>
}

impl Default for Response {
    fn default() -> Self {
        Response {
            status: 0,
            headers: Vec::with_capacity(20),
            body_reader: None
        }
    }
}

pub struct HttpClient {
    connections: HashMap<String, Connection>
}

impl HttpClient {
    pub async fn new() -> HttpClient {
        HttpClient { connections: HashMap::new() }
    }

    pub async fn perform_request(
        &mut self,
        url: &Url,
        request: Arc<ReadyRequest>
    ) -> Result<Response, Error> {
        let scheme = url.scheme();
        let host = url.host().expect("There must be domain").to_string();
        let use_tls = scheme == "https";
        let port = url.port().unwrap_or({
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

        match connection.send_request(request.clone()).await {
            Err(Error::ConnectionClosedUnexpectedly) => {
                self.connections.insert(
                    url.to_string(),
                    Connection::new(host.as_str(), port, use_tls, ConnectionOptions::default()).await?
                );
                self.connections.get_mut(url).unwrap().send_request(request).await
            },
            something_else => something_else
        }
    }
}