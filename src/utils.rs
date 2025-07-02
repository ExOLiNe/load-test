use std::collections::HashMap;
use std::net::{SocketAddr, ToSocketAddrs};
use lazy_static::lazy_static;
use log::{info};
use tokio::sync::Mutex;

use anyhow::{anyhow, Result};
use crate::error::MyError::IpResolve;

pub struct Statistics {
    times: HashMap<&'static str, (u128, u32)>,
}

impl Statistics {
    fn new() -> Self {
        Statistics {
            times: HashMap::new(),
        }
    }

    pub fn add_time(&mut self, function_name: &'static str, time: u128) {
        self.times.entry(function_name).and_modify(|(sum, count)| { *sum += time; *count += 1; }).or_insert((time, 1));
    }

    pub fn print(&mut self) {
        let divider = 1_000_000_f64;
        info!("Execution time statistics:");
        for (func, (sum, count)) in &self.times {
            info!("{}: total: {}, average: {}", func, *sum as f64 / divider, *sum as f64 / (*count as f64 * divider));
        }
    }
}

impl Drop for Statistics {
    fn drop(&mut self) {
        self.print();
    }
}

lazy_static! {
    pub static ref STATISTICS: Mutex<Statistics> = Mutex::new(Statistics::new());
}

#[macro_export]
macro_rules! measure_time {
    ($block:block) => {{
        let start = std::time::Instant::now();
        let result = $block;
        let duration = start.elapsed();
        let mut stats = $crate::utils::STATISTICS.lock().await;
        stats.add_time(concat!(file!(), ":", line!(), ":", stringify!($block)), duration.as_micros());
        result
    }};
}

pub const NEWLINE: &str = "\r\n";
pub const NEWLINE_BYTES: &[u8] = NEWLINE.as_bytes();

pub(crate) fn ip_resolve(host: &str, port: u16) -> Result<SocketAddr> {
    let addrs_iter = (host, port).to_socket_addrs()?;
    for addr in addrs_iter {
        if addr.is_ipv4() {
            return Ok(addr);
        }
    }
    Err(anyhow!(IpResolve))
}