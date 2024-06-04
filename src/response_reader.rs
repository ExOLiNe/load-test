use std::cmp::min;
use tokio::io::{AsyncBufReadExt, AsyncReadExt, BufReader, ReadHalf};
use tokio::net::TcpStream;
use bytes::BytesMut;
use crate::error::Error;
use crate::header::HttpHeader;

#[derive(Clone)]
enum ReaderState {
    ReadingStatus,
    ReadingHeaders,
    ReadingBody
}

pub enum HttpEntity {
    Status(u32),
    Header(HttpHeader),
    HeaderEnd,
    Body(String),
    End,
}

pub struct HttpResponseReader {
    reader: BufReader<ReadHalf<TcpStream>>,
    state: ReaderState,
    content_length_read: Option<(usize, usize)>
}

impl HttpResponseReader {
    pub fn new(reader: BufReader<ReadHalf<TcpStream>>) -> Self {
        HttpResponseReader {
            reader: reader,
            state: ReaderState::ReadingStatus,
            content_length_read: None
        }
    }

    pub async fn next_entity(&mut self) -> Result<HttpEntity, Error> {
        match self.state {
            ReaderState::ReadingStatus => {
                let mut status_str = String::new();
                match self.reader.read_line(&mut status_str).await {
                    Ok(0) => {
                        panic!("WTF????????");
                    }
                    Ok(_) => {
                        self.state = ReaderState::ReadingHeaders;
                        // todo impl parse status
                        Ok(HttpEntity::Status(200))
                    }
                    Err(e) => Err(Error::StdError(e)),
                }
            },
            ReaderState::ReadingHeaders => {
                let mut header = String::new();
                match self.reader.read_line(&mut header).await {
                    Ok(0) => {
                        panic!("WTF????????");
                    }
                    Ok(_) => {
                        if header == "\r\n" {
                            self.state = ReaderState::ReadingBody;
                            Ok(HttpEntity::HeaderEnd)
                            /*self.read_body().await.map(|str| {
                                str.map_or(HttpEntity::End, |str| {
                                    HttpEntity::Body(str)
                                })
                            })*/
                        } else {
                            // todo replace unwrap with ?
                            let header_parsed: HttpHeader = header.try_into().unwrap();
                            if header_parsed.name == "Content-Length" {
                                //todo replace unwrap with ?
                                let content_length: usize = header_parsed.value.parse().unwrap();
                                self.content_length_read = Some((0, content_length));
                            }
                            Ok(HttpEntity::Header(header_parsed))
                        }
                    }
                    Err(e) => Err(Error::StdError(e)),
                }
            }
            ReaderState::ReadingBody => {
                self.read_body().await.map(|str| {
                    str.map_or(HttpEntity::End, |str| {
                        HttpEntity::Body(str)
                    })
                })
            }
        }
    }

    async fn read_body(&mut self) -> Result<Option<String>, Error> {
        assert!(self.content_length_read.is_some());
        let (mut already_read, content_length) = self.content_length_read.unwrap();

        let to_read = content_length - already_read;

        if to_read == 0 {
            return Ok(None);
        }

        let to_read = min(to_read, 4096);

        let mut buf = BytesMut::with_capacity(to_read);
        match self.reader.read_buf(&mut buf).await {
            Ok(0) => {
                panic!("W T F CORPORATION");
            }
            Ok(size) => {
                already_read += size;
                self.content_length_read = Some((already_read, content_length));
                Ok(Some(String::from_utf8_lossy(&buf).to_string()))
            },
            Err(e) => {
                Err(Error::StdError(e))
            }
        }
    }
}

/*impl Clone for HttpResponseReader {
    fn clone(&self) -> Self {
        todo!()
    }

    fn clone_from(&mut self, source: &Self) {
        todo!()
    }
}*/