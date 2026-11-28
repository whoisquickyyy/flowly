use std::sync::Arc;

use futures_util::StreamExt;
use reqwest::Client;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::TcpListener;
use tokio::sync::RwLock;

use crate::models::{ProxyMode, SourceKind};
use crate::providers::{sc_worker_endpoint, MusicProvider, ProviderRegistry};

pub fn bind_proxy_listener() -> std::io::Result<(std::net::TcpListener, u16)> {
    let std_listener = std::net::TcpListener::bind("127.0.0.1:0")?;
    std_listener.set_nonblocking(true)?;
    let port = std_listener.local_addr()?.port();
    Ok((std_listener, port))
}

pub async fn run_proxy_server(
    std_listener: std::net::TcpListener,
    registry: Arc<RwLock<ProviderRegistry>>,
    cache: crate::track_cache::TrackCache,
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
        let cache = cache.clone();

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

            // URL на доменах SoundCloud (в т.ч. обложки с source=image):
            // в Direct-режиме напрямую они недоступны — как и весь SC-трафик
            // отправляем их через Worker. Ключ кэша остаётся исходным URL.
            let is_sc_host = url::Url::parse(&url)
                .ok()
                .map(|u| {
                    let h = u.host_str().unwrap_or("").to_ascii_lowercase();
                    h == "sndcdn.com"
                        || h.ends_with(".sndcdn.com")
                        || h == "soundcloud.com"
                        || h.ends_with(".soundcloud.com")
                })
                .unwrap_or(false);
            let fetch_url = if is_sc_host && source_str.as_deref() != Some("soundcloud") {
                let r = reg.read().await;
                if r.soundcloud.proxy_config().mode == ProxyMode::Direct {
                    sc_worker_endpoint(&url)
                } else {
                    url.clone()
                }
            } else {
                url.clone()
            };

            if let Some((cached_path, _mime)) = cache.get_cached(&url) {
                serve_local_file(&cached_path.to_string_lossy(), range_header.as_deref(), &mut writer).await;
                return;
            }

            let client = {
                let r = reg.read().await;
                match source_str.as_deref() {
                    Some("yandex") => r.yandex.client().clone(),
                    Some("soundcloud") => r.soundcloud.client().clone(),
                    // Обложки SoundCloud приходят с source=image на URL sndcdn.com
                    _ if is_sc_host => r.soundcloud.client().clone(),
                    _ => fb,
                }
            };

            // SoundCloud HLS: все треки теперь отдаются m3u8-плейлистами (fMP4-сегменты),
            // которые <audio> проиграть не может. Собираем init + сегменты в цельный MP4.
            if url.to_lowercase().contains(".m3u8") {
                let wrap_via_worker = {
                    let r = reg.read().await;
                    source_str.as_deref() == Some("soundcloud")
                        && r.soundcloud.proxy_config().mode == ProxyMode::Direct
                };
                if serve_hls_as_mp4(
                    &client,
                    &url,
                    wrap_via_worker,
                    range_header.as_deref(),
                    &cache,
                    &mut writer,
                )
                .await
                {
                    return;
                }
                // Плейлист не разобрался — падаем в обычную стриминговую ветку ниже.
            }

            // Первичный GET: до 3 попыток (нестабильный CDN/прокси не роняют трек сразу)
            let mut upstream = None;
            for attempt in 0..3u32 {
                if attempt > 0 {
                    tokio::time::sleep(std::time::Duration::from_millis(500 * attempt as u64)).await;
                }
                let mut req = client.get(&fetch_url);
                if let Some(ref rng) = range_header {
                    req = req.header("Range", rng);
                }
                match req.send().await {
                    Ok(r) => {
                        upstream = Some(r);
                        break;
                    }
                    Err(_) if attempt < 2 => continue,
                    Err(_) => {}
                }
            }
            let Some(mut upstream) = upstream else {
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

            let mut content_type = String::from("audio/mpeg");
            if let Some(ct) = upstream.headers().get("content-type").and_then(|v| v.to_str().ok()) {
                content_type = ct.to_string();
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

            let should_cache = status.as_u16() == 200 && range_header.is_none() && cache.is_enabled();
            let mut cache_buf: Vec<u8> = Vec::new();
            let mut cache_ok = should_cache;

            while let Ok(Some(chunk)) = upstream.chunk().await {
                if cache_ok {
                    if cache_buf.len() + chunk.len() <= 40 * 1024 * 1024 {
                        cache_buf.extend_from_slice(&chunk);
                    } else {
                        cache_ok = false;
                        cache_buf.clear();
                    }
                }
                if writer.write_all(&chunk).await.is_err() {
                    cache_ok = false;
                    break;
                }
            }
            if cache_ok && !cache_buf.is_empty() {
                cache.store_bytes(&url, &content_type, &cache_buf).await;
            }
        });
    }
}

#[allow(dead_code)]
fn _ensure_source_kind_used(_: SourceKind) {}

/// Скачивает один HLS-сегмент целиком. Обрыв соединения — не повод терять всю
/// сборку: до 3 попыток с бэкоффом.
async fn fetch_segment(client: &Client, url: String) -> Option<Vec<u8>> {
    for attempt in 0..3u32 {
        if attempt > 0 {
            tokio::time::sleep(std::time::Duration::from_millis(400 * attempt as u64)).await;
        }
        if let Some(resp) = client.get(&url).send().await.ok() {
            if let Ok(ok_r) = resp.error_for_status() {
                if let Ok(b) = ok_r.bytes().await {
                    return Some(b.to_vec());
                }
            }
        }
    }
    None
}

/// Загружает HLS-плейлист (VOD), скачивает init-сегмент и все медиа-сегменты
/// и склеивает их в цельный MP4 (fMP4-фрагменты конкатенируются корректно).
/// Используется и прокси-сервером (отдача клиенту), и фоновым прогревом
/// следующих треков (warm_up_next_tracks).
pub(crate) async fn assemble_hls_mp4(
    client: &Client,
    playlist_url: &str,
    wrap_via_worker: bool,
) -> Option<Vec<u8>> {
    // 1. Плейлист (маленький текстовый файл), 2 попытки
    let mut text = String::new();
    for attempt in 0..2u32 {
        if attempt > 0 {
            tokio::time::sleep(std::time::Duration::from_millis(500)).await;
        }
        if let Some(resp) = client.get(playlist_url).send().await.ok() {
            if let Ok(t) = resp.text().await {
                text = t;
                break;
            }
        }
    }
    if text.is_empty() {
        return None;
    }
    if !text.trim_start().starts_with("#EXTM3U") {
        return None;
    }

    // 2. Разбор плейлиста: EXT-X-MAP (init.mp4) + абсолютные URL сегментов
    let base = url::Url::parse(playlist_url).ok()?;
    let mut init_url: Option<String> = None;
    let mut segments: Vec<String> = Vec::new();
    for line in text.lines() {
        let line = line.trim();
        if let Some(rest) = line.strip_prefix("#EXT-X-MAP:URI=\"") {
            if let Some(u) = rest.split('"').next() {
                init_url = Some(u.to_string());
            }
        } else if !line.is_empty() && !line.starts_with('#') {
            if let Ok(abs) = base.join(line) {
                segments.push(abs.to_string());
            }
        }
    }
    if segments.is_empty() {
        return None;
    }

    // 3. В Direct-режиме сегменты тоже должны идти через Worker
    let wrap = |u: &str| -> String {
        if wrap_via_worker {
            sc_worker_endpoint(u)
        } else {
            u.to_string()
        }
    };

    let mut fetch_urls: Vec<String> = Vec::with_capacity(segments.len() + 1);
    if let Some(init) = &init_url {
        fetch_urls.push(wrap(init));
    }
    fetch_urls.extend(segments.iter().map(|s| wrap(s)));

    // 4. Параллельная загрузка сегментов (buffered сохраняет порядок)
    let parts: Vec<Option<Vec<u8>>> =
        futures_util::stream::iter(fetch_urls.into_iter().map(|u| fetch_segment(client, u)))
            .buffered(8)
            .collect()
            .await;

    if parts.iter().any(|p| p.is_none()) {
        return None;
    }

    // 5. Склейка init.mp4 + data*.m4s -> валидный MP4
    let mut buf: Vec<u8> = Vec::new();
    for part in parts.into_iter().flatten() {
        buf.extend_from_slice(&part);
    }
    if buf.len() < 1024 {
        return None;
    }
    Some(buf)
}

/// Загружает HLS-плейлист (VOD), собирает цельный MP4 и отдаёт клиенту
/// с поддержкой Range. Результат кэшируется под ключом playlist_url.
/// Возвращает true, если ответ полностью отдан клиенту.
#[allow(clippy::too_many_arguments)]
async fn serve_hls_as_mp4(
    client: &Client,
    playlist_url: &str,
    wrap_via_worker: bool,
    range_header: Option<&str>,
    cache: &crate::track_cache::TrackCache,
    writer: &mut tokio::net::tcp::WriteHalf<'_>,
) -> bool {
    let Some(buf) = assemble_hls_mp4(client, playlist_url, wrap_via_worker).await else {
        return false;
    };

    // Кэш (дедуп при повторном воспроизведении; лимиты внутри store_bytes)
    cache.store_bytes(playlist_url, "audio/mp4", &buf).await;

    // Отдача с Range-поддержкой
    serve_bytes(&buf, "audio/mp4", range_header, writer).await;
    true
}

/// Отдаёт буфер памяти как HTTP-ответ с поддержкой Range (аналог serve_local_file).
async fn serve_bytes(
    data: &[u8],
    mime: &str,
    range_header: Option<&str>,
    writer: &mut tokio::net::tcp::WriteHalf<'_>,
) {
    let total_len = data.len() as u64;
    if total_len == 0 {
        let _ = writer
            .write_all(b"HTTP/1.1 200 OK\r\nAccess-Control-Allow-Origin: *\r\nContent-Length: 0\r\nConnection: close\r\n\r\n")
            .await;
        return;
    }

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
    let _ = writer
        .write_all(&data[start as usize..=(end as usize)])
        .await;
}

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
