use std::env;
use std::rc::Rc;
use std::str::FromStr;
use std::time::Duration;
use log::debug;
use tokio::time::sleep;
use http_client::error::Error;

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
    debug!("Let's start!");
    let a = Rc::new(String::from_str(
        "sdf"
            ).unwrap());

    let mut vec = Vec::with_capacity(1_000_000);

    for _ in 0..vec.capacity() {
        vec.push(a.clone());
    }
    sleep(Duration::from_secs(1)).await;
    debug!("{:?}", &vec[args[1].as_str().parse::<usize>().unwrap()]);
    Ok(())
}