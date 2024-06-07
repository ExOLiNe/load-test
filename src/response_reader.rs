use std::cmp::{max, min};
use std::fs::File;
use std::io::Write;

use bytes::BytesMut;
use log::debug;
use tokio::io::{AsyncBufReadExt, AsyncReadExt};

use crate::error::{CommonError, Error};
use crate::header::HttpHeader;
use crate::response_reader::ResponseBodyType::Plain;
use crate::utils::NEWLINE;

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

type already_read = usize;
type content_length = usize;

enum ResponseBodyType {
    Plain((already_read, content_length)),
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
    // content_length_read: Option<(usize, usize)>
    response_body_type: ResponseBodyType
}

impl <T>HttpResponseReader<T>
where T : AsyncBufReadExt + Unpin
{
    pub fn new(reader: T) -> Self {
        HttpResponseReader {
            reader,
            state: ReaderState::ReadingStatus,
            response_body_type: ResponseBodyType::default()
        }
    }

    pub fn reset(&mut self) {
        self.state = ReaderState::ReadingStatus;
        self.response_body_type = ResponseBodyType::default();
    }

    pub async fn next_entity(&mut self) -> Result<HttpEntity, Error> {
        match self.state {
            ReaderState::ReadingStatus => {
                let mut status_str = String::new();
                match self.reader.read_line(&mut status_str).await {
                    Ok(0) => {
                        return Err(Error::Crate(CommonError { reason: String::from("WTF???") }));
                    }
                    Ok(_) => {
                        self.state = ReaderState::ReadingHeaders;
                        let mut status_iter = status_str.split(" ");
                        status_iter.next();
                        let status: u32 = status_iter.next().expect("could not find status").parse()?;
                        Ok(HttpEntity::Status(status))
                    }
                    Err(e) => Err(Error::IO(e)),
                }
            },
            ReaderState::ReadingHeaders => {
                let mut header = String::new();
                match self.reader.read_line(&mut header).await {
                    Ok(0) => {
                        panic!("WTF????????");
                    }
                    Ok(_) => {
                        if header == NEWLINE {
                            self.state = ReaderState::ReadingBody;
                            Ok(HttpEntity::HeaderEnd)
                        } else {
                            let header_parsed: HttpHeader = header.try_into()?;
                            if header_parsed.name == "Content-Length" {
                                let content_length: usize = header_parsed.value.parse()?;
                                self.response_body_type = Plain((0, content_length));
                            }
                            if header_parsed.name == "Transfer-Encoding" && header_parsed.value == "chunked" {
                                self.response_body_type = ResponseBodyType::Chunked;
                            }
                            Ok(HttpEntity::Header(header_parsed))
                        }
                    }
                    Err(e) => Err(Error::IO(e)),
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
        match self.response_body_type {
            Plain((mut already_read, content_length)) => {
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
                        self.response_body_type = Plain((already_read, content_length));
                        Ok(Some(String::from_utf8_lossy(&buf).to_string()))
                    },
                    Err(e) => {
                        Err(Error::IO(e))
                    }
                }
            }
            ResponseBodyType::Chunked => {
                let mut buf = String::with_capacity(65535);
                let mut body_result_buf = String::with_capacity(65535);
                let mut read_body_size = true;
                loop {
                    match self.reader.read_line(&mut buf).await {
                        Ok(_) => {
                            let buf_slice = buf.trim();
                            if read_body_size {
                                if buf_slice.len() > 0 {
                                    let chunk_size = usize::from_str_radix(buf_slice, 16).map_err(|err| {
                                        panic!();
                                    }).unwrap();
                                    if chunk_size == 0 {
                                        break;
                                    } else {
                                        let to_reserve = max(chunk_size as i32 - buf.capacity() as i32, 0) as usize;
                                        if to_reserve > 0 {
                                            buf.reserve(to_reserve);
                                        }
                                    }
                                }
                            } else {
                                body_result_buf += &buf_slice;
                            }
                            buf.clear();
                            read_body_size = !read_body_size;
                        },
                        Err(err) => {
                            panic!("{}", err);
                        }
                    };
                }

                return Ok(Some(body_result_buf));
            }
        }
    }
}