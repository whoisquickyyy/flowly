use std::collections::HashMap;
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr};
use std::sync::{Arc, OnceLock};
use std::time::{Duration, Instant};

use reqwest::dns::{Addrs, Name, Resolve, Resolving};
use reqwest::Client;
use serde::Deserialize;
use tokio::sync::RwLock;

#[derive(Clone)]
struct DohRuntimeConfig {
    enabled: bool,
    provider: String,
}

static DOH_CONFIG: OnceLock<std::sync::RwLock<DohRuntimeConfig>> = OnceLock::new();
static DOH_CACHE: OnceLock<Arc<RwLock<HashMap<String, CachedEntry>>>> = OnceLock::new();

fn config_lock() -> &'static std::sync::RwLock<DohRuntimeConfig> {
    DOH_CONFIG.get_or_init(|| {
        std::sync::RwLock::new(DohRuntimeConfig {
            enabled: true,
            provider: "cloudflare".to_string(),
        })
    })
}

fn shared_cache() -> Arc<RwLock<HashMap<String, CachedEntry>>> {
    Arc::clone(DOH_CACHE.get_or_init(|| Arc::new(RwLock::new(HashMap::new()))))
}

pub fn set_doh_config(enabled: bool, provider: &str) {
    if let Ok(mut cfg) = config_lock().write() {
        cfg.enabled = enabled;
        cfg.provider = provider.to_string();
    }
    if let Ok(mut cache) = shared_cache().try_write() {
        cache.clear();
    }
}

pub fn get_doh_config() -> (bool, String) {
    config_lock()
        .read()
        .map(|c| (c.enabled, c.provider.clone()))
        .unwrap_or((true, "cloudflare".to_string()))
}

pub fn endpoint_for_provider(provider: &str) -> &'static str {
    match provider {
        "google" => "https://dns.google/dns-query",
        "cloudflare" => "https://cloudflare-dns.com/dns-query",
        "quad9" => "https://dns.quad9.net/dns-query",
        "controld" => "https://freedns.controld.com/p0",
        "dns4eu" => "https://unfiltered.joindns4.eu/dns-query",
        "yandex" => "https://common.dot.dns.yandex.net/dns-query",
        "xbox" => "https://xbox-dns.ru/dns-query",
        "comss" => "https://dns.comss.one/dns-query",
        "dns-ai" => "https://dns.dns-ai.ru/dns-query",
        "bezmezhau" => "https://sync.bezmezhau.com/dns-query",
        "geohide" => "https://geohide.cc/dns-query",
        "malw" => "https://dns.malw.link/dns-query",
        "webshift" => "https://d.webshiftvpn.ru/dns-query",
        "dagami" => "https://dns.dagami.ru/dns-query",
        "magicdns" => "https://dns.magicdns.ru/dns-query",
        "tamaev" => "https://tamaev-gifts.xyz/dns-query",
        "zombiez" => "https://dns.zombiez.ru/dns-query",
        "wlrus" => "https://dns.wlrus.cfd/dns-query",
        "magicinfo" => "https://api.magicinfo.art/dns-query",
        "quicbest" => "https://w.quic.best/dns-query",
        _ => "https://cloudflare-dns.com/dns-query",
    }
}

#[derive(Debug, Deserialize)]
struct DohResponse {
    #[serde(rename = "Answer")]
    answer: Option<Vec<DohAnswer>>,
}

#[derive(Debug, Deserialize)]
struct DohAnswer {
    #[serde(rename = "type")]
    record_type: u16,
    data: String,
    #[serde(rename = "TTL")]
    ttl: Option<u64>,
}

struct CachedEntry {
    ips: Vec<IpAddr>,
    expires_at: Instant,
}

fn build_dns_a_query(host: &str) -> Vec<u8> {
    let mut pkt = Vec::with_capacity(64);
    // Header: ID=0x1234, RD=1, QDCOUNT=1
    pkt.extend_from_slice(&[0x12, 0x34, 0x01, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00]);
    for label in host.trim_end_matches('.').split('.') {
        let bytes = label.as_bytes();
        if bytes.is_empty() || bytes.len() > 63 {
            continue;
        }
        pkt.push(bytes.len() as u8);
        pkt.extend_from_slice(bytes);
    }
    pkt.push(0); // root
    pkt.extend_from_slice(&[0x00, 0x01, 0x00, 0x01]); // QTYPE=A, QCLASS=IN
    pkt
}

fn skip_dns_name(buf: &[u8], mut pos: usize) -> Option<usize> {
    let mut jumps = 0;
    loop {
        let len = *buf.get(pos)?;
        if len == 0 {
            return Some(pos + 1);
        }
        if (len & 0xC0) == 0xC0 {
            return Some(pos + 2);
        }
        pos += 1 + len as usize;
        jumps += 1;
        if jumps > 64 {
            return None;
        }
    }
}

fn parse_dns_wire_response(buf: &[u8]) -> Option<(Vec<IpAddr>, u64)> {
    if buf.len() < 12 {
        return None;
    }
    let qdcount = u16::from_be_bytes([buf[4], buf[5]]);
    let ancount = u16::from_be_bytes([buf[6], buf[7]]);
    let mut pos = 12;
    for _ in 0..qdcount {
        pos = skip_dns_name(buf, pos)?;
        pos += 4; // QTYPE + QCLASS
    }
    let mut ips = Vec::new();
    let mut min_ttl = 300u64;
    for _ in 0..ancount {
        pos = skip_dns_name(buf, pos)?;
        if pos + 10 > buf.len() {
            break;
        }
        let rtype = u16::from_be_bytes([buf[pos], buf[pos + 1]]);
        let ttl = u32::from_be_bytes([buf[pos + 4], buf[pos + 5], buf[pos + 6], buf[pos + 7]]) as u64;
        let rdlength = u16::from_be_bytes([buf[pos + 8], buf[pos + 9]]) as usize;
        pos += 10;
        if pos + rdlength > buf.len() {
            break;
        }
        if rtype == 1 && rdlength == 4 {
            ips.push(IpAddr::V4(Ipv4Addr::new(
                buf[pos],
                buf[pos + 1],
                buf[pos + 2],
                buf[pos + 3],
            )));
            min_ttl = min_ttl.min(ttl.max(30));
        } else if rtype == 28 && rdlength == 16 {
            let mut octets = [0u8; 16];
            octets.copy_from_slice(&buf[pos..pos + 16]);
            ips.push(IpAddr::V6(Ipv6Addr::from(octets)));
            min_ttl = min_ttl.min(ttl.max(30));
        }
        pos += rdlength;
    }
    if ips.is_empty() {
        None
    } else {
        Some((ips, min_ttl))
    }
}

/// Встроенный DNS-over-HTTPS (DoH) резолвер с поддержкой RFC 8484 wire-format и JSON API.
pub struct DohResolver {
    doh_client: Client,
    cache: Arc<RwLock<HashMap<String, CachedEntry>>>,
}

impl DohResolver {
    pub fn new() -> Self {
        let doh_client = Client::builder()
            .timeout(Duration::from_secs(4))
            .no_proxy()
            .build()
            .unwrap_or_else(|_| Client::new());
        Self {
            doh_client,
            cache: shared_cache(),
        }
    }

    async fn query_doh_endpoint(&self, endpoint: &str, host: &str) -> Option<(Vec<IpAddr>, u64)> {
        // 1. Сначала пробуем RFC 8484 wire-format (поддерживается всеми DoH серверами)
        let wire_query = build_dns_a_query(host);
        if let Ok(resp) = self
            .doh_client
            .post(endpoint)
            .header("Content-Type", "application/dns-message")
            .header("Accept", "application/dns-message")
            .body(wire_query)
            .send()
            .await
        {
            if resp.status().is_success() {
                if let Ok(bytes) = resp.bytes().await {
                    if let Some(parsed) = parse_dns_wire_response(&bytes) {
                        return Some(parsed);
                    }
                }
            }
        }

        // 2. Fallback на JSON DoH API (если сервер предпочитает application/dns-json)
        let resp = self
            .doh_client
            .get(endpoint)
            .query(&[("name", host), ("type", "A")])
            .header("Accept", "application/dns-json")
            .send()
            .await
            .ok()?
            .error_for_status()
            .ok()?;

        let json: DohResponse = resp.json().await.ok()?;
        let mut ips = Vec::new();
        let mut min_ttl = 300u64;

        for ans in json.answer.unwrap_or_default() {
            if ans.record_type == 1 || ans.record_type == 28 {
                if let Ok(ip) = ans.data.parse::<IpAddr>() {
                    ips.push(ip);
                    if let Some(t) = ans.ttl {
                        min_ttl = min_ttl.min(t.max(30));
                    }
                }
            }
        }

        if ips.is_empty() {
            None
        } else {
            Some((ips, min_ttl))
        }
    }

    pub async fn test_provider(&self, provider: &str) -> Result<u64, String> {
        let endpoint = endpoint_for_provider(provider);
        let started = Instant::now();
        self.query_doh_endpoint(endpoint, "example.com")
            .await
            .map(|_| started.elapsed().as_millis() as u64)
            .ok_or_else(|| "Нет ответа".to_string())
    }

    async fn resolve_host(
        &self,
        host: &str,
    ) -> Result<Vec<IpAddr>, Box<dyn std::error::Error + Send + Sync>> {
        if let Ok(ip) = host.parse::<IpAddr>() {
            return Ok(vec![ip]);
        }

        if host.eq_ignore_ascii_case("localhost") {
            return Ok(vec![IpAddr::from([127, 0, 0, 1])]);
        }

        {
            let cache = self.cache.read().await;
            if let Some(entry) = cache.get(host) {
                if Instant::now() < entry.expires_at {
                    return Ok(entry.ips.clone());
                }
            }
        }

        let (doh_enabled, provider) = get_doh_config();
        if doh_enabled {
            let primary = endpoint_for_provider(&provider);
            let doh_res = match self.query_doh_endpoint(primary, host).await {
                Some(r) => Some(r),
                None => {
                    let fallback = if primary == "https://dns.google/dns-query" {
                        "https://cloudflare-dns.com/dns-query"
                    } else {
                        "https://dns.google/dns-query"
                    };
                    self.query_doh_endpoint(fallback, host).await
                }
            };

            if let Some((ips, ttl)) = doh_res {
                let mut cache = self.cache.write().await;
                cache.insert(
                    host.to_string(),
                    CachedEntry {
                        ips: ips.clone(),
                        expires_at: Instant::now() + Duration::from_secs(ttl),
                    },
                );
                return Ok(ips);
            }
        }

        // Fallback на системный DNS
        let addrs: Vec<IpAddr> = tokio::net::lookup_host((host, 0))
            .await?
            .map(|sa| sa.ip())
            .collect();
        if addrs.is_empty() {
            return Err("DNS lookup returned no addresses".into());
        }
        Ok(addrs)
    }
}

impl Resolve for DohResolver {
    fn resolve(&self, name: Name) -> Resolving {
        let host = name.as_str().to_string();
        let doh_client = self.doh_client.clone();
        let cache = Arc::clone(&self.cache);

        Box::pin(async move {
            let resolver = DohResolver { doh_client, cache };
            let ips = resolver.resolve_host(&host).await?;
            let addrs: Addrs = Box::new(ips.into_iter().map(|ip| SocketAddr::new(ip, 0)));
            Ok(addrs)
        })
    }
}
