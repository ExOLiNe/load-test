use std::collections::HashMap;
use std::{env, fs};
use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::str::FromStr;
use std::sync::Arc;
use futures::future;
use log::{debug};
use tokio::fs::read_to_string;
use http_client::client::HttpClient;
use http_client::request::{Method, Request};
use anyhow::Result;
use serde::Deserialize;
use url::Url;

#[cfg(feature = "dhat-heap")]
#[global_allocator]
static ALLOC: dhat::Alloc = dhat::Alloc;

#[derive(Deserialize, Debug)]
pub struct LoadTestRequest {
    pub request: RequestData,
    pub repeats: usize,
    pub max_connections: usize
}

#[derive(Deserialize, Debug)]
pub struct RequestData {
    pub query: String,
    pub method: String,
    pub headers: HashMap<String, String>,
    pub body: Option<PathBuf>
}

pub fn to_request(data: &RequestData, working_dir: &Path) -> Result<Request> {
    Ok(Request {
        method: Method::from_str(&data.method)?,
        url: Url::parse(&data.query)?,
        headers: data.headers.clone(),
        body: data.body.clone().map(|body| {
            read_to_body(working_dir.join("body").join(body)).unwrap()
        })
    })
}

pub(crate) fn read_to_body(path: PathBuf) -> Result<Arc<Pin<String>>> {
    Ok(Arc::new(Pin::new(fs::read_to_string(path)?)))
}

#[tokio::main]
async fn main() -> Result<()> {
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

        let url = Arc::new(request.url.clone());

        let ready_request = Arc::new(request.get_raw().await);

        let mut handles = Vec::with_capacity(req_data.max_connections);

        let before = std::time::SystemTime::now();
        debug!("Start");

        for _ in 0..req_data.max_connections {
            let url = url.clone();
            let mut client = HttpClient::new().await;
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
                        }
                        debug!("Read body: {}", String::from_utf8_lossy(&response_body));
                    }
                }
            }));
        }

        future::join_all(handles).await;

        println!("Time spent: {}", before.elapsed().unwrap().as_millis());
    }
    // STATISTICS.lock().await.print();
    Ok(())
}