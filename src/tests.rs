#[cfg(test)]
mod tests {
    use std::sync::Arc;
    use futures::future;
    use tokio::fs::read_to_string;
    use log::{debug, info};
    use tokio::io::{BufReader, ReadHalf};
    use tokio::net::TcpStream;
    use crate::client::{HttpClient, Response};
    use crate::error::Error;
    use crate::header::HttpHeader;
    use crate::load_test_request::{LoadTestRequest};
    use crate::request::{Request};
    use crate::response_reader::HttpResponseReader;
    use crate::utils::NEWLINE;

    #[tokio::test]
    async fn test_http_client() -> Result<(), Error> {
        env_logger::init();

        let data: Vec<LoadTestRequest> = serde_yaml::from_str(
            read_to_string("D:\\RustroverProjects\\http_client\\test_data\\request\\request.yml").await?.as_mut_str()
        )?;

        for req_data in data {
            let requests_per_connection = req_data.repeats / req_data.max_connections;

            let mut request: Request = (&req_data.request).try_into()?;

            let ready_request = Arc::new(request.get_raw().await);

            let mut handles = Vec::with_capacity(req_data.max_connections);

            let before = std::time::SystemTime::now();

            for _ in 0..req_data.max_connections {
                let mut client = HttpClient::new().await;
                let url = request.url.clone();
                let ready_request = ready_request.clone();
                handles.push(tokio::spawn(async move {
                    for _ in 0..requests_per_connection {
                        let response = client.perform_request(&url, ready_request.clone()).await?;
                        response.headers.iter().for_each(|header| {
                            debug!("{}", header);
                        });
                        debug!("{}", NEWLINE);
                        if let Some(mut body_reader) = response.body_reader {
                            loop {
                                if let Some(buf) = body_reader.recv().await {
                                    debug!("{}", buf);
                                } else {
                                    break;
                                }
                            }
                        }
                    }
                    Ok::<(), Error>(())
                }));
            }

            future::join_all(handles).await;

            let after = std::time::SystemTime::now();

            info!("Time spent: {}", after.duration_since(before)?.as_secs());
        }

        /*let connections = 20;
        let requests_per_connection = 150;
        let mut headers = HashMap::new();
        headers.insert(String::from("X-Profile-Id"), String::from("000001"));
        headers.insert(String::from("X-Session-Id"), String::from("000001"));
        headers.insert(String::from("Content-Type"), String::from("application/json"));

        let profile = read_to_string("D:\\RustroverProjects\\http_client\\body.json").await.unwrap();

        let mut request = Request {
            method: Method::POST,
            url: "http://127.0.0.1:8081/v1/test?profileVersion=1.0.0&commitId=1000".parse().expect("Invalid url"),
            headers: headers,
            body: profile.chars()
        };*/
        Ok(())
    }

    #[test]
    fn normal_types() {
        crate::error::is_normal::<Error>();
        crate::error::is_normal::<HttpHeader>();
        crate::error::is_normal::<Request>();
        crate::error::is_normal::<Response>();
        crate::error::is_normal::<HttpClient>();
        crate::error::is_normal::<HttpResponseReader<BufReader<ReadHalf<TcpStream>>>>();
    }
}