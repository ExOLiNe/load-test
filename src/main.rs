use std::net::{IpAddr, Ipv4Addr, SocketAddr, ToSocketAddrs};
use std::process::exit;
use std::sync::Arc;
use futures::future;
use log::{debug, info};
use tokio::fs::read_to_string;
use http_client::client::HttpClient;
use http_client::error::Error;
use http_client::load_test_request::LoadTestRequest;
use http_client::request::Request;

#[tokio::main]
async fn main() -> Result<(), Error> {
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

        for i in 0..req_data.max_connections {
            let mut client = HttpClient::new().await;
            let url = request.url.clone();
            let ready_request = ready_request.clone();
            handles.push(tokio::spawn(async move {
                for _ in 0..requests_per_connection {
                    let response = client.perform_request(&url, ready_request.clone()).await?;
                    debug!("Connect: {}", i);
                    if let Some(mut body_reader) = response.body_reader {
                        loop {
                            if let Some(buf) = body_reader.recv().await {
                                debug!("{}", buf);
                            } else {
                                break;
                            }
                        }
                    }
                    debug!("Finished");
                }
                Ok::<(), Error>(())
            }));
        }

        future::join_all(handles).await;

        let after = std::time::SystemTime::now();

        println!("Time spent: {}", after.duration_since(before)?.as_millis());
    }
    Ok(())
}