use std::net::{SocketAddr, SocketAddrV4};
use std::sync::Arc;

use tokio::io::{self, AsyncWriteExt, BufReader, WriteHalf};
use tokio::net::{TcpSocket, TcpStream};
use tokio::sync::Mutex;

use crate::client::Response;
use crate::error::Error;
use crate::request::ReadyRequest;
use crate::response_reader::{HttpEntity, HttpResponseReader};

pub(crate) struct Connection {
    writer: WriteHalf<TcpStream>,
    reader: Option<HttpResponseReader>,
    pub(crate) in_progress: Arc<Mutex<bool>>,
    buf: Vec<u8>
}

impl Connection {
    pub async fn write_request(&mut self, request: Arc<ReadyRequest>) -> Result<(), io::Error> {
        let mut in_progress = self.in_progress.lock().await;
        *in_progress = true;
        let writer = &mut self.writer;
        writer.write_all(request.0.as_bytes()).await?;
        writer.write_all(request.1.as_bytes()).await?;
        writer.write_all("\n".as_bytes()).await?;
        writer.write_all("\n".as_bytes()).await?;
        Ok(())
    }

    pub async fn read_response(&mut self) -> Result<Response, Error> {
        println!("Read response");
        let before = std::time::SystemTime::now();
        let mut response = Response::default();
        loop {
            match self.reader.as_mut().expect("is not None").next_entity().await {
                Ok(value) => {
                    match value {
                        HttpEntity::Status(status) => {
                            response.status = status;
                        }
                        HttpEntity::Header(header) => {
                            response.headers.push(header);
                        }
                        HttpEntity::HeaderEnd => {
                            break;
                        }
                        _ => ()
                    }
                },
                Err(e) => {
                    panic!("{:?}", e);
                }
            }
        }

        let (sender, receiver) = tokio::sync::mpsc::channel(1024);

        let in_progress = self.in_progress.clone();

        response.body_reader = Some(receiver);

        if let Some(mut reader) = self.reader.take() {
            println!("Take reader!");
            tokio::spawn(async move {
                loop {
                    match reader.next_entity().await {
                        Ok(value) => {
                            match value {
                                HttpEntity::Status(_) | HttpEntity::HeaderEnd | HttpEntity::Header(_) => (),
                                HttpEntity::Body(body) => {
                                    sender.send(body).await.unwrap();
                                }
                                HttpEntity::End => {
                                    // println!("end");
                                    break
                                }
                            }
                        },
                        Err(e) => {
                            panic!("{:?}", e);
                        }
                    };
                }
                let mut in_progress = in_progress.lock().await;
                *in_progress = false;
            });
        }

        let after = std::time::SystemTime::now();

        // println!("Reading response time: {}", after.duration_since(before).unwrap().as_millis());

        Ok(response)
    }
}

impl Connection {
    #[async_backtrace::framed]
    pub async fn new(addr: SocketAddrV4) -> Result<Connection, Error> {
        let socket = TcpSocket::new_v4().expect("Socket creation error");
        let (reader, writer) = io::split(
            socket.connect(SocketAddr::V4(addr)).await.map_err(|err| {
                Error::StdError(err)
            })?
        );
        Ok(Connection {
            writer: writer,
            reader: Some(HttpResponseReader::new(BufReader::new(reader))),
            in_progress: Arc::new(Mutex::new(false)),
            buf: vec![0; 10]
        })
    }
}