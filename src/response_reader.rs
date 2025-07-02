use bytes::{Buf, Bytes, BytesMut};
use tokio::io::{AsyncBufRead, AsyncBufReadExt, AsyncReadExt};

use crate::header::HttpHeader;
use crate::response_reader::ResponseBodyType::{Chunked, Plain};
use crate::utils::NEWLINE;
use anyhow::{anyhow, Result};
use crate::error::MyError::{ConnectionClosedUnexpectedly, HeaderParseError, ZeroRead};

#[derive(Clone)]
enum ReaderState {
    Status,
    Headers,
    Body
}

#[derive(Debug)]
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
where T : AsyncBufRead + Unpin
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

    pub async fn next_entity(&mut self) -> Result<HttpEntity> {
        match self.state {
            ReaderState::Status => {
                let mut status_str = String::with_capacity(256);
                match self.reader.read_line(&mut status_str).await {
                    Ok(0) => {
                        Err(anyhow!(ConnectionClosedUnexpectedly))
                    }
                    Ok(_) => {
                        self.state = ReaderState::Headers;
                        let mut status_iter = status_str.split(' ');
                        status_iter.next();
                        let status: u32 = status_iter.next().ok_or(HeaderParseError)?.parse()?;
                        Ok(HttpEntity::Status(status))
                    }
                    Err(e) => {
                        Err(anyhow!(e))
                    },
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
                            if header_parsed.name.eq_ignore_ascii_case("Content-Length") {
                                let content_length: usize = header_parsed.value.parse()?;
                                self.response_body_type = Plain((0, content_length));
                            }
                            if header_parsed.name.eq_ignore_ascii_case("Transfer-Encoding") && header_parsed.value.eq_ignore_ascii_case("chunked") {
                                self.response_body_type = Chunked;
                            }
                            Ok(HttpEntity::Header(header_parsed))
                        }
                    }
                    Err(e) => {
                        Err(anyhow!(e))
                    },
                }
            }
            ReaderState::Body => {
                self.read_body_next().await.map(|str: Option<Bytes>| {
                    str.map_or(HttpEntity::End, |body| {
                        HttpEntity::Body(body)
                    })
                })
            }
        }
    }

    async fn read_body_next(&mut self) -> Result<Option<Bytes>> {
        match self.response_body_type {
            Plain((already_read, content_length)) => {
                if already_read == content_length {
                    return Ok(None);
                }
                let to_read = content_length - already_read;
                while self.buf.len() < to_read {
                    let read = self.reader.read_buf(&mut self.buf).await?;
                    if read == 0 { return Err(anyhow!(ZeroRead)); }
                }
                let chunk = self.buf.split_to(to_read).freeze();
                self.response_body_type = Plain((already_read + to_read, content_length));
                Ok(Some(chunk))
            }
            Chunked => {
                let mut chunk_size_buf = String::new();
                loop {
                    let newline_pos = self.buf.windows(2).position(|w| w == b"\r\n");
                    if let Some(pos) = newline_pos {
                        // Found \r\n — extract line
                        chunk_size_buf.push_str(&String::from_utf8_lossy(&self.buf[..pos]));
                        self.buf.advance(pos + 2); // Consume line + CRLF
                        break;
                    }

                    // Need more data
                    let read = self.reader.read_buf(&mut self.buf).await?;
                    if read == 0 {
                        return Err(anyhow!("EOF while reading chunk size line"));
                    }
                }

                let chunk_size = usize::from_str_radix(chunk_size_buf.trim(), 16)?;
                if chunk_size == 0 {
                    // Consume final CRLF
                    loop {
                        if self.buf.windows(2).position(|w| w == b"\r\n").is_some() {
                            self.buf.advance(2);
                            break;
                        }
                        let read = self.reader.read_buf(&mut self.buf).await?;
                        if read == 0 {
                            return Err(anyhow!("EOF while reading final CRLF"));
                        }
                    }
                    return Ok(None);
                }

                // Wait until we have enough bytes for the chunk + CRLF
                let total_needed = chunk_size + 2;
                while self.buf.len() < total_needed {
                    let read = self.reader.read_buf(&mut self.buf).await?;
                    if read == 0 {
                        return Err(anyhow!("EOF while reading chunk body"));
                    }
                }

                // Split out chunk
                let chunk = self.buf.split_to(chunk_size).freeze();

                // Validate and consume CRLF
                if &self.buf[..2] != b"\r\n" {
                    return Err(anyhow!("Missing CRLF after chunk"));
                }
                self.buf.advance(2);

                Ok(Some(chunk))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::{fs, io};
    use std::path::Path;
    use std::sync::Once;
    use log::{debug};
    use tokio::io::BufReader;
    use anyhow::{anyhow, Result};
    use strum_macros::Display;
    use crate::response_reader::{HttpEntity, HttpResponseReader};
    use crate::response_reader::tests::TestError::ResponseHasNotRead;

    static INIT: Once = Once::new();

    #[derive(Debug)]
    struct TestCase {
        name: String,
        request: Option<String>,
        response: Option<String>
    }

    #[derive(Debug, Display)]
    enum TestError {
        ResponseHasNotRead
    }

    fn load_test_case(dir: &Path) -> io::Result<TestCase> {
        let request_path = dir.join("request");
        let response_path = dir.join("response");

        let name = dir
            .file_name()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_else(|| "<unknown>".to_string());

        let request = fs::read_to_string(&request_path).ok();
        let response = fs::read_to_string(&response_path).ok();

        Ok(TestCase { name, request, response })
    }

    #[tokio::test]
    async fn test_plain() -> Result<()> {
        response_reader_test(load_test_case(Path::new("test_resources/plain"))?).await
    }

    #[tokio::test]
    async fn test_chunked() -> Result<()> {
        response_reader_test(load_test_case(Path::new("test_resources/chunked"))?).await
    }

    async fn response_reader_test(case: TestCase) -> Result<()> {
        INIT.call_once(|| {
            env_logger::builder()
                .is_test(true)
                .filter(None, log::LevelFilter::Debug)
                .format_timestamp_millis().init();
        });
        let mut body_already_read = false;
        if let Some(response) = case.response {
            let mut reader = HttpResponseReader::new(BufReader::new(response.as_bytes()));
            loop {
                let entity = reader.next_entity().await?;
                if let HttpEntity::Body(_) = entity {
                    body_already_read = true;
                }
                if let HttpEntity::End = entity {
                    if !body_already_read {
                        return Err(anyhow!(ResponseHasNotRead));
                    }
                    break;
                }
            }
        }
        Ok(())
    }
}