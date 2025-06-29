use std::env;
use std::path::PathBuf;
use std::sync::Arc;
use futures::future;
use log::{debug};
use tokio::fs::read_to_string;
use http_client::client::HttpClient;
use http_client::error::Error;
use http_client::load_test_request::{LoadTestRequest, to_request};
use http_client::request::Request;
use http_client::utils::STATISTICS;

#[cfg(feature = "dhat-heap")]
#[global_allocator]
static ALLOC: dhat::Alloc = dhat::Alloc;

#[tokio::main]
async fn main() -> Result<(), Error> {
    #[cfg(feature = "dhat-heap")]
    let _profiler = dhat::Profiler::new_heap();

    env_logger::builder()
        .filter(None, log::LevelFilter::Debug)
        .format_timestamp_millis().init();

    let args: Vec<String> = env::args().collect();

    if args.len() < 2 {
        panic!("Usage: {} <path>", args[0]);
    }

    let path = PathBuf::from(&args[1]);

    let path = if path.is_absolute() {
        path
    } else {
        env::current_dir().unwrap().join(path)
    };

    let data: Vec<LoadTestRequest> = serde_yaml::from_str(
        read_to_string(path.join("request.yml").to_str().unwrap()).await?.as_mut_str()
    )?;

    for req_data in data {
        let requests_per_connection = req_data.repeats / req_data.max_connections;

        let mut request: Request = to_request(&req_data.request, &path)?;

        let ready_request = Arc::new(request.get_raw().await);

        let mut handles = Vec::with_capacity(req_data.max_connections);

        let before = std::time::SystemTime::now();
        debug!("Start");

        for _ in 0..req_data.max_connections {
            let mut client = HttpClient::new().await;
            let url = request.url.clone();
            let ready_request = ready_request.clone();
            debug!("{:?}", &ready_request.0);
            handles.push(tokio::spawn(async move {
                for _ in 0..requests_per_connection {
                    debug!("=======================================================================");
                    let response = client.perform_request(&url, ready_request.clone()).await.unwrap();
                    debug!("Read headers: {:?}", response.headers);
                    if let Some(mut body_reader) = response.body_reader {
                        let mut response_body: Vec<u8> = Vec::new();
                        while let Some(buf) = body_reader.recv().await {
                            response_body.extend_from_slice(&buf);
                            // debug!("{}", buf);
                        }
                        debug!("Read body: {}", String::from_utf8_lossy(&response_body));
                    }
                    // sleep(Duration::from_secs(3)).await;
                }
            }));
        }

        future::join_all(handles).await;

        println!("Time spent: {}", before.elapsed().unwrap().as_millis());
    }
    STATISTICS.lock().await.print();
    Ok(())
}