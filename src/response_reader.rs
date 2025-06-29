use bytes::{Bytes, BytesMut};
use tokio::io::{AsyncBufReadExt, AsyncReadExt};

use crate::error::Error;
use crate::error::Error::ZeroRead;
use crate::header::HttpHeader;
use crate::response_reader::ResponseBodyType::{Chunked, Plain};
use crate::utils::NEWLINE;

#[derive(Clone)]
enum ReaderState {
    Status,
    Headers,
    Body
}

pub enum HttpEntity {
    Status(u32),
    Header(HttpHeader),
    HeaderEnd,
    Body(Bytes),
    End,
}

type AlreadyRead = usize;
type ContentLength = usize;

enum ResponseBodyType {
    Plain((AlreadyRead, ContentLength)),
    Chunked
}

impl Default for ResponseBodyType {
    fn default() -> Self {
        Plain((0, 0))
    }
}

pub struct HttpResponseReader<T> {
    reader: T,
    state: ReaderState,
    response_body_type: ResponseBodyType,
    buf: BytesMut
}

impl <T>HttpResponseReader<T>
where T : AsyncBufReadExt + Unpin
{
    pub fn new(reader: T) -> Self {
        HttpResponseReader {
            reader,
            state: ReaderState::Status,
            response_body_type: ResponseBodyType::default(),
            buf: BytesMut::with_capacity(1024)
        }
    }

    pub fn reset(&mut self) {
        self.state = ReaderState::Status;
        self.response_body_type = ResponseBodyType::default();
        self.buf.clear();
    }

    pub async fn next_entity(&mut self) -> Result<HttpEntity, Error> {
        match self.state {
            ReaderState::Status => {
                let mut status_str = String::with_capacity(256);
                match self.reader.read_line(&mut status_str).await {
                    Ok(0) => {
                        Err(ZeroRead)
                    }
                    Ok(_) => {
                        self.state = ReaderState::Headers;
                        let mut status_iter = status_str.split(' ');
                        status_iter.next();
                        let status: u32 = status_iter.next().ok_or(Error::ParseStatus)?.parse()?;
                        Ok(HttpEntity::Status(status))
                    }
                    Err(e) => Err(Error::IO(e)),
                }
            },
            ReaderState::Headers => {
                let mut header = String::with_capacity(512);
                let read_line = self.reader.read_line(&mut header).await;
                match read_line {
                    Ok(0) => {
                        panic!("WTF????????");
                    }
                    Ok(_) => {
                        if header == NEWLINE {
                            self.state = ReaderState::Body;
                            Ok(HttpEntity::HeaderEnd)
                        } else {
                            let header_parsed: HttpHeader = header.try_into()?;
                            if header_parsed.name == "Content-Length" {
                                let content_length: usize = header_parsed.value.parse()?;
                                self.response_body_type = Plain((0, content_length));
                            }
                            if header_parsed.name == "Transfer-Encoding" && header_parsed.value == "chunked" {
                                self.response_body_type = Chunked;
                            }
                            Ok(HttpEntity::Header(header_parsed))
                        }
                    }
                    Err(e) => Err(Error::IO(e)),
                }
            }
            ReaderState::Body => {
                self.read_body_next().await.map(|str| {
                    str.map_or(HttpEntity::End, |body| {
                        HttpEntity::Body(body)
                    })
                })
            }
        }
    }

    async fn read_body_next(&mut self) -> Result<Option<Bytes>, Error> {
        match self.response_body_type {
            Plain((already_read, content_length)) => {
                if already_read == content_length {
                    return Ok(None);
                }
                let to_read = content_length - already_read;
                while self.buf.len() < to_read {
                    let read = self.reader.read_buf(&mut self.buf).await?;
                    if read == 0 { return Err(ZeroRead); }
                }
                let chunk = self.buf.split_to(to_read).freeze();
                self.response_body_type = Plain((already_read + to_read, content_length));
                Ok(Some(chunk))
            }
            Chunked => {
                let mut chunk_size_buf = String::with_capacity(8);

                self.reader.read_line(&mut chunk_size_buf).await?;
                let chunk_size = usize::from_str_radix(&chunk_size_buf, 16)?;
                if chunk_size == 0 {
                    let mut trailing = String::new();
                    self.reader.read_line(&mut trailing).await?;
                    // TODO
                    return Ok(None);
                }

                while self.buf.len() < chunk_size {
                    let read = self.reader.read_buf(&mut self.buf).await?;
                    if read == 0 { return Err(ZeroRead); }
                }

                let chunk = self.buf.split_to(chunk_size).freeze();
                let mut crlf = String::new();
                self.reader.read_line(&mut crlf).await?;

                Ok(Some(chunk))
            }
        }
    }
}