use std::net::{IpAddr, SocketAddr};

use axum::http::HeaderMap;

pub const FORWARDED_FOR: &str = "x-forwarded-for";

pub const REAL_IP: &str = "x-real-ip";

pub fn client_ip(headers: &HeaderMap, remote: IpAddr, trust_proxy: bool) -> IpAddr {
    if !trust_proxy {
        return remote;
    }

    forwarded_for(headers)
        .or_else(|| header_ip(headers, REAL_IP))
        .unwrap_or(remote)
}

fn forwarded_for(headers: &HeaderMap) -> Option<IpAddr> {
    let raw = headers.get(FORWARDED_FOR)?.to_str().ok()?;
    raw.split(',').find_map(|part| parse_ip(part.trim()))
}

fn header_ip(headers: &HeaderMap, name: &str) -> Option<IpAddr> {
    let raw = headers.get(name)?.to_str().ok()?;
    parse_ip(raw.trim())
}

fn parse_ip(text: &str) -> Option<IpAddr> {
    if text.is_empty() {
        return None;
    }

    if let Ok(address) = text.parse::<IpAddr>() {
        return Some(address);
    }

    text.parse::<SocketAddr>().ok().map(|address| address.ip())
}

#[cfg(test)]
mod tests {
    use super::*;

    const REMOTE: &str = "10.0.0.1";

    fn remote() -> IpAddr {
        REMOTE.parse().expect("адрес разбирается")
    }

    fn headers(pairs: &[(&str, &str)]) -> HeaderMap {
        let mut headers = HeaderMap::new();
        for (name, value) in pairs {
            headers.insert(
                axum::http::HeaderName::from_bytes(name.as_bytes()).expect("имя верное"),
                value.parse().expect("значение верное"),
            );
        }
        headers
    }

    #[test]
    fn chain_gives_the_first_address() {
        let map = headers(&[(FORWARDED_FOR, "203.0.113.7, 10.0.0.2")]);
        let address = client_ip(&map, remote(), true);
        assert_eq!(address.to_string(), "203.0.113.7");
    }

    #[test]
    fn real_ip_is_the_fallback() {
        let map = headers(&[(REAL_IP, "203.0.113.8")]);
        let address = client_ip(&map, remote(), true);
        assert_eq!(address.to_string(), "203.0.113.8");
    }

    #[test]
    fn address_with_port_is_understood() {
        let map = headers(&[(FORWARDED_FOR, "203.0.113.9:51234")]);
        let address = client_ip(&map, remote(), true);
        assert_eq!(address.to_string(), "203.0.113.9");
    }

    #[test]
    fn without_trust_headers_are_ignored() {
        let map = headers(&[(FORWARDED_FOR, "203.0.113.7")]);
        let address = client_ip(&map, remote(), false);
        assert_eq!(address, remote());
    }

    #[test]
    fn garbage_falls_back_to_the_socket() {
        let map = headers(&[(FORWARDED_FOR, "не адрес")]);
        let address = client_ip(&map, remote(), true);
        assert_eq!(address, remote());
    }
}
