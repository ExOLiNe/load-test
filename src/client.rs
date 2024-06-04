use std::collections::HashMap;
use std::net::SocketAddrV4;
use std::str::FromStr;
use std::sync::Arc;

use tokio::sync::mpsc::Receiver;
use url::Url;

use crate::connection::Connection;
use crate::error::{CommonError, Error};
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
    connections: HashMap<String, Vec<Connection>>
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
        let port = url.port().unwrap_or_else(|| {
            if scheme == "https://" {
                443u16
            } else {
                80u16
            }
        });

        let connection = match self.find_connection(host.as_str()).await {
            Some(conn) => conn,
            None => {
                let url = SocketAddrV4::from_str(
                    format!("{}:{}", host, port).as_str()
                ).map_err(|err| {
                    Error::OwnError(CommonError { reason: err.to_string() })
                })?;
                self.create_new_connection(url).await?;
                self.find_connection(host.as_str()).await.expect("Unexpected error")
            }
        };

        connection.write_request(request).await.unwrap();
        let response = connection.read_response().await.unwrap();
        Ok(response)
    }

    async fn find_connection(&mut self, host: &str) -> Option<&mut Connection> {
        return if let Some(pool) = self.connections.get_mut(host) {
            for connection in pool {
                if !*connection.in_progress.lock().await {
                    return Some(connection)
                }
            }
            None
        } else {
            None
        }
    }

    #[async_backtrace::framed]
    async fn create_new_connection(&mut self, addr: SocketAddrV4) -> Result<(), Error> {
        let new_connection = Connection::new(addr).await?;
        let host: String = addr.ip().to_string();
        let connections_by_host = self.connections.get_mut(&host);
        match connections_by_host {
            Some(connections) => connections.push(new_connection),
            None => {
                assert!(self.connections.insert(host.clone(), vec![new_connection]).is_none());
                self.connections.get(host.as_str()).ok_or_else(|| {
                    Error::OwnError(CommonError { reason: String::from("Unexpected behaviour") })
                })?;
            }
        }
        Ok(())
    }
}