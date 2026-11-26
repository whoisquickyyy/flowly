use std::sync::Arc;

use reqwest::Client;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::TcpListener;
use tokio::sync::RwLock;

use crate::models::SourceKind;
use crate::providers::{MusicProvider, ProviderRegistry};

pub fn bind_proxy_listener() -> std::io::Result<(std::net::TcpListener, u16)> {
    let std_listener = std::net::TcpListener::bind("127.0.0.1:0")?;
    std_listener.set_nonblocking(true)?;
    let port = std_listener.local_addr()?.port();
    Ok((std_listener, port))
}

pub async fn run_proxy_server(
    std_listener: std::net::TcpListener,
    registry: Arc<RwLock<ProviderRegistry>>,
) {
    let listener = match TcpListener::from_std(std_listener) {
        Ok(l) => l,
        Err(e) => {
            eprintln!("Failed to start audio proxy listener: {}", e);
            return;
        }
    };

    let fallback_client = Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .user_agent("Mozilla/5.0")
        .build()
        .unwrap_or_else(|_| Client::new());

    loop {
        let Ok((mut socket, _)) = listener.accept().await else {
            continue;
        };
        let reg = Arc::clone(&registry);
        let fb = fallback_client.clone();

        tokio::spawn(async move {
            let (reader, mut writer) = socket.split();
            let mut buf_reader = BufReader::new(reader);

            let mut request_line = String::new();
            if buf_reader.read_line(&mut request_line).await.unwrap_or(0) == 0 {
                return;
            }

            let mut parts = request_line.split_whitespace();
            let method = parts.next().unwrap_or("GET");
            let path = parts.next().unwrap_or("/");

            let mut range_header: Option<String> = None;
            loop {
                let mut line = String::new();
                if buf_reader.read_line(&mut line).await.unwrap_or(0) == 0 {
                    break;
                }
                let trimmed = line.trim();
                if trimmed.is_empty() {
                    break;
                }
                if let Some((k, v)) = trimmed.split_once(':') {
                    if k.eq_ignore_ascii_case("range") {
                        range_header = Some(v.trim().to_string());
                    }
                }
            }

            if method.eq_ignore_ascii_case("OPTIONS") {
                let resp = "HTTP/1.1 204 No Content\r\n\
                    Access-Control-Allow-Origin: *\r\n\
                    Access-Control-Allow-Methods: GET, HEAD, OPTIONS\r\n\
                    Access-Control-Allow-Headers: Range\r\n\
                    Connection: close\r\n\r\n";
                let _ = writer.write_all(resp.as_bytes()).await;
                return;
            }

            let Ok(parsed) = url::Url::parse(&format!("http://127.0.0.1{}", path)) else {
                let _ = writer
                    .write_all(b"HTTP/1.1 400 Bad Request\r\nAccess-Control-Allow-Origin: *\r\nConnection: close\r\n\r\n")
                    .await;
                return;
            };

            if parsed.path() == "/local" {
                let mut local_path: Option<String> = None;
                for (k, v) in parsed.query_pairs() {
                    if k == "path" {
                        local_path = Some(v.to_string());
                    }
                }
                let Some(file_path) = local_path else {
                    let _ = writer
                        .write_all(b"HTTP/1.1 400 Bad Request\r\nAccess-Control-Allow-Origin: *\r\nConnection: close\r\n\r\n")
                        .await;
                    return;
                };
                serve_local_file(&file_path, range_header.as_deref(), &mut writer).await;
                return;
            }

            let mut target_url: Option<String> = None;
            let mut source_str: Option<String> = None;
            for (k, v) in parsed.query_pairs() {
                if k == "url" {
                    target_url = Some(v.to_string());
                } else if k == "source" {
                    source_str = Some(v.to_string());
                }
            }

            let Some(url) = target_url else {
                let _ = writer
                    .write_all(b"HTTP/1.1 400 Bad Request\r\nAccess-Control-Allow-Origin: *\r\nConnection: close\r\n\r\n")
                    .await;
                return;
            };

            let client = {
                let r = reg.read().await;
                match source_str.as_deref() {
                    Some("yandex") => r.yandex.client().clone(),
                    Some("spotify") => r.spotify.client().clone(),
                    Some("soundcloud") => r.soundcloud.client().clone(),
                    _ => fb,
                }
            };

            let mut req = client.get(&url);
            if let Some(ref rng) = range_header {
                req = req.header("Range", rng);
            }

            let Ok(mut upstream) = req.send().await else {
                let _ = writer
                    .write_all(b"HTTP/1.1 502 Bad Gateway\r\nAccess-Control-Allow-Origin: *\r\nConnection: close\r\n\r\n")
                    .await;
                return;
            };

            let status = upstream.status();
            let status_line = format!(
                "HTTP/1.1 {} {}\r\n",
                status.as_u16(),
                status.canonical_reason().unwrap_or("OK")
            );

            let mut headers = String::from(&status_line);
            headers.push_str("Access-Control-Allow-Origin: *\r\n");
            headers.push_str("Access-Control-Allow-Headers: Range\r\n");
            headers.push_str("Access-Control-Expose-Headers: Content-Length, Content-Range, Accept-Ranges\r\n");
            headers.push_str("Accept-Ranges: bytes\r\n");
            headers.push_str("Connection: close\r\n");

            if let Some(ct) = upstream.headers().get("content-type").and_then(|v| v.to_str().ok()) {
                headers.push_str(&format!("Content-Type: {}\r\n", ct));
            }
            if let Some(cl) = upstream.headers().get("content-length").and_then(|v| v.to_str().ok()) {
                headers.push_str(&format!("Content-Length: {}\r\n", cl));
            }
            if let Some(cr) = upstream.headers().get("content-range").and_then(|v| v.to_str().ok()) {
                headers.push_str(&format!("Content-Range: {}\r\n", cr));
            }
            headers.push_str("\r\n");

            if writer.write_all(headers.as_bytes()).await.is_err() {
                return;
            }

            while let Ok(Some(chunk)) = upstream.chunk().await {
                if writer.write_all(&chunk).await.is_err() {
                    break;
                }
            }
        });
    }
}

#[allow(dead_code)]
fn _ensure_source_kind_used(_: SourceKind) {}

async fn serve_local_file(
    file_path: &str,
    range_header: Option<&str>,
    writer: &mut tokio::net::tcp::WriteHalf<'_>,
) {
    use tokio::io::{AsyncReadExt, AsyncSeekExt};

    let Ok(mut file) = tokio::fs::File::open(file_path).await else {
        let _ = writer
            .write_all(b"HTTP/1.1 404 Not Found\r\nAccess-Control-Allow-Origin: *\r\nConnection: close\r\n\r\n")
            .await;
        return;
    };
    let Ok(meta) = file.metadata().await else {
        return;
    };
    let total_len = meta.len();
    if total_len == 0 {
        let _ = writer
            .write_all(b"HTTP/1.1 200 OK\r\nAccess-Control-Allow-Origin: *\r\nContent-Length: 0\r\nConnection: close\r\n\r\n")
            .await;
        return;
    }

    let ext = std::path::Path::new(file_path)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    let mime = match ext.as_str() {
        "mp3" => "audio/mpeg",
        "flac" => "audio/flac",
        "wav" => "audio/wav",
        "ogg" | "opus" => "audio/ogg",
        "m4a" => "audio/mp4",
        "aac" => "audio/aac",
        "webm" => "audio/webm",
        _ => "application/octet-stream",
    };

    let (start, end, is_partial) = if let Some(rng) = range_header {
        if let Some(spec) = rng.strip_prefix("bytes=") {
            let mut parts = spec.split('-');
            let s = parts.next().and_then(|v| v.parse::<u64>().ok()).unwrap_or(0);
            let e = parts
                .next()
                .and_then(|v| if v.is_empty() { None } else { v.parse::<u64>().ok() })
                .unwrap_or(total_len - 1)
                .min(total_len - 1);
            if s <= e && s < total_len {
                (s, e, true)
            } else {
                (0, total_len - 1, false)
            }
        } else {
            (0, total_len - 1, false)
        }
    } else {
        (0, total_len - 1, false)
    };

    let content_len = end - start + 1;
    let mut headers = String::new();
    if is_partial {
        headers.push_str("HTTP/1.1 206 Partial Content\r\n");
        headers.push_str(&format!("Content-Range: bytes {}-{}/{}\r\n", start, end, total_len));
    } else {
        headers.push_str("HTTP/1.1 200 OK\r\n");
    }
    headers.push_str("Access-Control-Allow-Origin: *\r\n");
    headers.push_str("Access-Control-Allow-Headers: Range\r\n");
    headers.push_str("Access-Control-Expose-Headers: Content-Length, Content-Range, Accept-Ranges\r\n");
    headers.push_str("Accept-Ranges: bytes\r\n");
    headers.push_str(&format!("Content-Type: {}\r\n", mime));
    headers.push_str(&format!("Content-Length: {}\r\n", content_len));
    headers.push_str("Connection: close\r\n\r\n");

    if writer.write_all(headers.as_bytes()).await.is_err() {
        return;
    }

    if start > 0 && file.seek(std::io::SeekFrom::Start(start)).await.is_err() {
        return;
    }

    let mut remaining = content_len;
    let mut buf = [0u8; 64 * 1024];
    while remaining > 0 {
        let to_read = (remaining as usize).min(buf.len());
        match file.read(&mut buf[..to_read]).await {
            Ok(0) | Err(_) => break,
            Ok(n) => {
                if writer.write_all(&buf[..n]).await.is_err() {
                    break;
                }
                remaining -= n as u64;
            }
        }
    }
}
