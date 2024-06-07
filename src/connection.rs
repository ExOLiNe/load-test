use std::cmp::min;
use std::sync::Arc;
use std::time::{Duration, SystemTime};
use log::{debug, warn};

use tokio::io::{self, AsyncRead, AsyncWriteExt, BufReader, ReadHalf, WriteHalf};
use tokio::net::{TcpSocket, TcpStream as TokioTcpStream};
use tokio::sync::Mutex;
use tokio_native_tls::{TlsConnector, TlsStream as TokioTlsStream};

use crate::client::Response;
use crate::error::Error;
use crate::request::ReadyRequest;
use crate::response_reader::{HttpEntity, HttpResponseReader};
use crate::utils::{ip_resolve, NEWLINE_BYTES};

type ResponseReader<T> = Arc<Mutex<HttpResponseReader<BufReader<ReadHalf<T>>>>>;

enum StreamWriter {
    Plain(WriteHalf<TokioTcpStream>),
    Tls(WriteHalf<TokioTlsStream<TokioTcpStream>>)
}

enum StreamReader {
    Plain(ResponseReader<TokioTcpStream>),
    Tls(ResponseReader<TokioTlsStream<TokioTcpStream>>)
}

#[derive(Clone, Copy)]
pub struct ConnectionOptions {
    idle_timeout: Duration
}

impl Default for ConnectionOptions {
    fn default() -> Self {
        ConnectionOptions { idle_timeout: Duration::from_secs(10) }
    }
}

pub(crate) struct Connection {
    options: ConnectionOptions,
    writer: StreamWriter,
    reader: StreamReader,
    pub(crate) in_progress: Arc<Mutex<bool>>
}

impl Connection {
    pub async fn write<T: AsyncWriteExt + Unpin>(writer: &mut T, request: &Arc<ReadyRequest>, in_progress: Arc<Mutex<bool>>) -> Result<(), Error> {
        let mut in_progress = in_progress.lock().await;
        *in_progress = true;
        writer.write_all(request.0.as_bytes()).await?;
        match &request.1 {
            Some(body) => {
                writer.write_all(body.as_bytes()).await?
            },
            None => ()
        };
        writer.write_all(NEWLINE_BYTES).await?;
        writer.write_all(NEWLINE_BYTES).await?;
        Ok(())
    }

    pub async fn read<T>(
        reader: ResponseReader<T>,
        in_progress: Arc<Mutex<bool>>,
        options: ConnectionOptions
    ) -> Result<Response, Error>
    where T: AsyncRead + Unpin + Send + 'static
    {
        let mut last_packet_time = SystemTime::now();
        let mut response = Response::default();
        {
            let mut reader = reader.lock().await;
            loop {
                match reader.next_entity().await {
                    Ok(value) => {
                        last_packet_time = SystemTime::now();
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
                            _ => panic!("Invalid state")
                        }
                    },
                    Err(_) => {
                        let idle = SystemTime::now().duration_since(last_packet_time).unwrap();
                        if idle > options.idle_timeout {
                            warn!("Idle timeout");
                            return Err(Error::IdleTimeout);
                        }
                    }
                }
            }
        }

        let (sender, receiver) = tokio::sync::mpsc::channel(1_000_000);

        let in_progress = in_progress.clone();

        response.body_reader = Some(receiver);

        let reader = reader.clone();
        tokio::spawn(async move {
            let mut reader = reader.lock().await;
            loop {
                match reader.next_entity().await {
                    Ok(value) => {
                        match value {
                            HttpEntity::Status(_) | HttpEntity::HeaderEnd | HttpEntity::Header(_) => (),
                            HttpEntity::Body(body) => {
                                sender.send(body).await?;
                            }
                            HttpEntity::End => {
                                break
                            }
                        }
                    },
                    Err(e) => {
                        panic!("{:?}", e);
                    }
                };
            }
            reader.reset();
            let mut in_progress = in_progress.lock().await;
            *in_progress = false;
            Ok::<(), Error>(())
        });

        Ok(response)
    }

    pub async fn send_request(&mut self, request: Arc<ReadyRequest>) -> Result<Response, Error> {
        match &mut self.writer {
            StreamWriter::Plain(writer) => {
                Connection::write(writer, &request, self.in_progress.clone()).await?;
            },
            StreamWriter::Tls(writer) => {
                Connection::write(writer, &request, self.in_progress.clone()).await?;
            }
        }

        let response = match &self.reader {
            StreamReader::Plain(reader) => {
                Connection::read(Arc::clone(reader), self.in_progress.clone(), self.options).await?
            },
            StreamReader::Tls(reader) => {
                Connection::read(Arc::clone(reader), self.in_progress.clone(), self.options).await?
            }
        };
        Ok(response)
    }
}

impl Connection {
    #[async_backtrace::framed]
    pub async fn new(host: &str, port: u16, use_tls: bool, options: ConnectionOptions) -> Result<Connection, Error> {
        let addr_v4 = ip_resolve(host, port)?;
        let (reader, writer) = {
            let socket = TcpSocket::new_v4()?;
            if use_tls {
                let native_tls_connector = native_tls::TlsConnector::builder()
                    .danger_accept_invalid_certs(true)
                    .danger_accept_invalid_hostnames(true)
                    .build().unwrap();
                let tls_connector = TlsConnector::from(native_tls_connector);
                let tcp_stream = socket.connect(addr_v4).await.expect(format!("could not connect to {}", addr_v4).as_str());
                let (reader, writer) =
                    io::split(tls_connector.connect(format!("{}:{}", addr_v4.ip(), addr_v4.port()).as_str(), tcp_stream).await?);
                (
                    StreamReader::Tls(
                        Arc::new(
                            Mutex::new(
                                HttpResponseReader::new(BufReader::new(reader))
                            )
                        )
                    ),
                    StreamWriter::Tls(writer)
                )
            } else {
                debug!("Plain tcp detected");
                let (reader, writer) = io::split(
                    socket.connect(addr_v4).await?
                );
                (
                    StreamReader::Plain(
                        Arc::new(
                            Mutex::new(
                                HttpResponseReader::new(BufReader::new(reader))
                            )
                        )
                    ),
                    StreamWriter::Plain(writer)
                )
            }
        };
        Ok(Connection {
            options,
            writer,
            reader,
            in_progress: Arc::new(Mutex::new(false))
        })
    }
}