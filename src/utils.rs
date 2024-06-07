use std::net::{SocketAddr, ToSocketAddrs};

use crate::error::Error;

pub const NEWLINE: &str = "\r\n";
pub const NEWLINE_BYTES: &[u8] = NEWLINE.as_bytes();

pub(crate) fn ip_resolve(host: &str, port: u16) -> Result<SocketAddr, Error> {
    let addrs_iter = (host, port).to_socket_addrs().map_err(
        |err| Error::IpResolve(format!("{}", err))
    )?;
    for addr in addrs_iter {
        if addr.is_ipv4() {
            return Ok(addr);
        }
    }
    return Err(Error::IpResolve(String::from("Could not resolve as ip4")));
}