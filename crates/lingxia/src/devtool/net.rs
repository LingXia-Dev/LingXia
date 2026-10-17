//! Reaching the machine that runs the dev session.
//!
//! An iOS host dials the Mac by its mDNS name, which survives a DHCP change and
//! may resolve to several addresses (IPv6 link-local among them). The Wi-Fi
//! address it had at build time rides along as `fallback=` for networks that
//! drop multicast.

use std::io;
use std::net::{IpAddr, SocketAddr, TcpStream, ToSocketAddrs};
use std::time::Duration;

/// Connect to the host of `url` (`ws://`, `wss://`, `http://`, or `https://`),
/// trying each address it resolves to, then its `fallback=` address.
pub fn connect_dev_server(url: &str, timeout: Duration) -> io::Result<TcpStream> {
    let (host, port) = host_and_port(url)
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, format!("bad URL {url}")))?;
    let mut last_error = None;
    for addr in candidates(&host, port, fallback(url).as_deref()) {
        match TcpStream::connect_timeout(&addr, timeout) {
            Ok(stream) => return Ok(stream),
            Err(err) => last_error = Some(err),
        }
    }
    Err(last_error.unwrap_or_else(|| {
        io::Error::new(
            io::ErrorKind::NotFound,
            format!("{host} resolved to no address"),
        )
    }))
}

fn candidates(host: &str, port: u16, fallback: Option<&str>) -> Vec<SocketAddr> {
    let mut addrs: Vec<SocketAddr> = match host.parse::<IpAddr>() {
        Ok(ip) => vec![SocketAddr::new(ip, port)],
        // An unresolvable name is not fatal while a fallback remains.
        Err(_) => (host, port)
            .to_socket_addrs()
            .map(Iterator::collect)
            .unwrap_or_default(),
    };
    if let Some(ip) = fallback.and_then(|value| value.parse::<IpAddr>().ok()) {
        let addr = SocketAddr::new(ip, port);
        if !addrs.contains(&addr) {
            addrs.push(addr);
        }
    }
    addrs
}

fn host_and_port(url: &str) -> Option<(String, u16)> {
    let (scheme, rest) = url.split_once("://")?;
    let authority = rest.split(['/', '?']).next()?;
    let default_port = match scheme {
        "wss" | "https" => 443,
        _ => 80,
    };
    if let Some(rest) = authority.strip_prefix('[') {
        let (host, tail) = rest.split_once(']')?;
        let port = match tail.strip_prefix(':') {
            Some(port) => port.parse().ok()?,
            None => default_port,
        };
        return Some((host.to_string(), port));
    }
    match authority.rsplit_once(':') {
        Some((host, port)) => Some((host.to_string(), port.parse().ok()?)),
        None => Some((authority.to_string(), default_port)),
    }
}

fn fallback(url: &str) -> Option<String> {
    let (_, query) = url.split_once('?')?;
    query.split('&').find_map(|pair| {
        let (key, value) = pair.split_once('=')?;
        (key == "fallback" && !value.is_empty()).then(|| value.to_string())
    })
}

#[cfg(test)]
mod tests {
    use super::{candidates, fallback, host_and_port};

    #[test]
    fn a_dev_url_names_its_host_port_and_fallback() {
        let url = "ws://mac.local:39231/?fallback=192.168.1.14&token=abc";
        assert_eq!(host_and_port(url), Some(("mac.local".to_string(), 39231)));
        assert_eq!(fallback(url).as_deref(), Some("192.168.1.14"));
        assert_eq!(
            host_and_port("http://[fe80::1]:9/__lingxia/dev"),
            Some(("fe80::1".to_string(), 9))
        );
        assert_eq!(fallback("ws://127.0.0.1:9"), None);
    }

    #[test]
    fn an_unresolvable_name_still_tries_the_fallback() {
        let addrs = candidates("no-such-host.invalid", 39231, Some("192.168.1.14"));
        assert_eq!(addrs, vec!["192.168.1.14:39231".parse().unwrap()]);
        let literal = candidates("127.0.0.1", 9, Some("127.0.0.1"));
        assert_eq!(literal.len(), 1);
    }
}
