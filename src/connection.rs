use std::sync::Arc;
use std::time::{Duration, SystemTime};
use log::{debug, warn};
use tokio::io::{self, AsyncRead, AsyncWriteExt, BufReader, ReadHalf, WriteHalf};
use tokio::net::{TcpSocket, TcpStream as TokioTcpStream};
use tokio::sync::Mutex;
use tokio_native_tls::{TlsConnector, TlsStream as TokioTlsStream};

use crate::client::Response;
use crate::request::ReadyRequest;
use crate::response_reader::{HttpEntity, HttpResponseReader};
use crate::utils::{ip_resolve, NEWLINE_BYTES};
use crate::constants::IDLE_TIMEOUT;
use crate::measure_time;
use anyhow::{Result, Error, anyhow};
use crate::error::MyError;
use crate::error::MyError::ConnectionClosedUnexpectedly;

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
        ConnectionOptions { idle_timeout: Duration::from_secs(IDLE_TIMEOUT) }
    }
}

pub(crate) struct Connection {
    options: ConnectionOptions,
    writer: StreamWriter,
    reader: StreamReader,
    pub(crate) in_progress: Arc<Mutex<bool>>
}

impl Connection {
    pub async fn write<T: AsyncWriteExt + Unpin>(writer: &mut T, request: &Arc<ReadyRequest>, in_progress: Arc<Mutex<bool>>) -> Result<()> {
        let mut in_progress = in_progress.lock().await;
        *in_progress = true;
        debug!("write headers: {:?}", request.0);
        writer.write_all(&*request.0).await?;
        if let Some(body) = &request.1 {
            writer.write_all(body.as_bytes()).await?
        }
        writer.write_all(NEWLINE_BYTES).await?;
        writer.write_all(NEWLINE_BYTES).await?;
        Ok(())
    }

    pub async fn read<T>(
        reader: ResponseReader<T>,
        in_progress: Arc<Mutex<bool>>,
        options: ConnectionOptions
    ) -> Result<Response>
    where T: AsyncRead + Unpin + Send + 'static
    {
        let mut last_packet_time = SystemTime::now();
        let mut response = Response::default();
        {
            let mut reader = reader.lock().await;
            reader.reset();
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
                    Err(error) => {
                        return match error.downcast::<MyError>() {
                            Ok(MyError::ZeroRead) => {
                                Err(anyhow!(ConnectionClosedUnexpectedly))
                            },
                            Ok(else_error) => {
                                Err(anyhow!(else_error))
                            }
                            Err(error) => {
                                let idle = SystemTime::now().duration_since(last_packet_time)?;
                                if idle > options.idle_timeout {
                                    warn!("Idle timeout");
                                    Err(anyhow!(MyError::IdleTimeout))
                                } else {
                                    Err(anyhow!(error))
                                }
                            }
                        }
                    }
                }
            }
        }

        let (sender, receiver) = tokio::sync::mpsc::channel(10 * 1024);

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

    pub async fn send_request(&mut self, request: Arc<ReadyRequest>) -> Result<Response> {
        debug!("send request");
        match &mut self.writer {
            StreamWriter::Plain(writer) => {
                Connection::write(writer, &request, self.in_progress.clone()).await?;
            },
            StreamWriter::Tls(writer) => {
                Connection::write(writer, &request, self.in_progress.clone()).await?;
            }
        }
        debug!("send request finished");

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

    pub async fn new(host: &str, port: u16, use_tls: bool, options: ConnectionOptions) -> Result<Connection> {
        let addr_v4 = measure_time!({
            ip_resolve(host, port)?
        });
        let (reader, writer) = {
            let socket = TcpSocket::new_v4()?;
            socket.set_keepalive(true)?;

            if use_tls {
                let native_tls_connector = native_tls::TlsConnector::builder()
                    .danger_accept_invalid_certs(true)
                    .danger_accept_invalid_hostnames(true)
                    .build()?;
                let tls_connector = TlsConnector::from(native_tls_connector);
                debug!("Connecting raw tcp..");
                let tcp_stream = socket.connect(addr_v4).await?;
                debug!("TCP connected");
                debug!("TLS Handshaking..");
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
                debug!("Connecting raw tcp..");
                let stream = socket.connect(addr_v4).await?;
                debug!("TCP connected");
                let (reader, writer) = io::split(
                    stream
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