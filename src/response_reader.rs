use bytes::BytesMut;
use tokio::io::{AsyncBufReadExt, AsyncReadExt};

use crate::error::Error;
use crate::error::Error::ZeroRead;
use crate::header::HttpHeader;
use crate::measure_time;
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
    Body(String),
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
            buf: BytesMut::with_capacity(1024 * 1024)
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
                let read_line = measure_time!({
                    self.reader.read_line(&mut header).await
                });
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
                    str.map_or(HttpEntity::End, |str| {
                        HttpEntity::Body(str)
                    })
                })
            }
        }
    }

    async fn read_body_next(&mut self) -> Result<Option<String>, Error> {
        match self.response_body_type {
            Plain((mut already_read, content_length)) => {
                let to_read = content_length - already_read;

                if to_read == 0 {
                    return Ok(None);
                }

                // let mut buf = BytesMut::with_capacity(to_read);
                match self.reader.read_buf(&mut self.buf).await {
                    Ok(0) => {
                        panic!("W T F CORPORATION");
                    }
                    Ok(size) => {
                        already_read += size;
                        self.response_body_type = Plain((already_read, content_length));
                        Ok(Some(String::from_utf8_lossy(&self.buf).to_string()))
                    },
                    Err(e) => {
                        Err(Error::IO(e))
                    }
                }
            }
            Chunked => {
                let mut chunk_size_buf = String::with_capacity(8);

                let chunk_size = {
                    measure_time!({
                            self.reader.read_line(&mut chunk_size_buf).await?
                        }
                    );

                    let buf_slice = chunk_size_buf.trim();
                    if !buf_slice.is_empty() {
                        let chunk_size = usize::from_str_radix(buf_slice, 16)?;
                        if chunk_size == 0 {
                            // read last \r\n\r\n in request
                            chunk_size_buf.clear();
                            self.reader.read_line(&mut chunk_size_buf).await.unwrap();
                            return Ok(None)
                        } else {
                            chunk_size
                        }
                    } else {
                        panic!();
                    }
                };
                unsafe {
                    measure_time!({
                        self.buf.clear();
                        self.buf.set_len(chunk_size)
                    });
                }
                let read_exact = measure_time!({
                    self.reader.read_exact(&mut self.buf[..chunk_size]).await
                });
                match read_exact {
                    Ok(size) => {
                        if size != chunk_size {
                            panic!("{} != {}", size, chunk_size);
                        }

                        // read \n\r after every chunk was read
                        chunk_size_buf.clear();
                        measure_time!({
                            self.reader.read_line(&mut chunk_size_buf).await?
                        });

                        let str = String::from_utf8_lossy(&self.buf[..chunk_size]).to_string();

                        Ok(Some(str))
                    },
                    Err(err) => {
                        panic!("{}", err);
                    }
                }
            }
        }
    }
}