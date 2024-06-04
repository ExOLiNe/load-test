#[cfg(test)]
mod tests {

    use std::collections::HashMap;
    use std::sync::Arc;
    use futures::future;
    use tokio::fs::read_to_string;
    use crate::client::{HttpClient};
    use crate::request::{Request, Method};

    #[tokio::test]
    async fn test_http_client() {
        let connections = 1;
        let requests_per_connection = 2;
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
        };
        let ready_request = Arc::new(request.get_raw().await);

        let mut handles = Vec::with_capacity(1000);
        for _ in 0..handles.capacity() {
            let mut client = HttpClient::new().await;
            let url = request.url.clone();
            let ready_request = ready_request.clone();
            handles.push(tokio::spawn(async move {
                for _ in 0..requests_per_connection {
                    let response = client.perform_request(&url, ready_request.clone()).await.unwrap();

                    if let Some(mut body_reader) = response.body_reader {
                        loop {
                            if let Some(buf) = body_reader.recv().await {
                                // println!("{}", buf);
                            } else {
                                break;
                            }
                        }
                    }
                }
            }));
        }

        future::join_all(handles).await;
    }
}