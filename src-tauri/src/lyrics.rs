use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::models::{SourceKind, Track};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LyricsLine {
    pub time_ms: Option<u64>,
    pub text: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LyricsResponse {
    pub synced: bool,
    pub source: String,
    pub lines: Vec<LyricsLine>,
}

fn parse_lrc(lrc: &str) -> Vec<LyricsLine> {
    let mut lines = Vec::new();
    for raw in lrc.lines() {
        let trimmed = raw.trim();
        if trimmed.is_empty() {
            continue;
        }
        // Парсим все [mm:ss.xx] теги в начале строки
        let mut rest = trimmed;
        let mut timestamps = Vec::new();
        while rest.starts_with('[') {
            let Some(end) = rest.find(']') else {
                break;
            };
            let tag = &rest[1..end];
            if let Some(ms) = parse_lrc_timestamp(tag) {
                timestamps.push(ms);
            }
            rest = rest[end + 1..].trim_start();
        }
        let text = rest.trim().to_string();
        if text.is_empty() {
            continue;
        }
        if timestamps.is_empty() {
            // Пропускаем мета-теги типа [ar:...] [ti:...]
            if trimmed.starts_with('[') && trimmed.ends_with(']') {
                continue;
            }
            lines.push(LyricsLine {
                time_ms: None,
                text,
            });
        } else {
            for ts in timestamps {
                lines.push(LyricsLine {
                    time_ms: Some(ts),
                    text: text.clone(),
                });
            }
        }
    }
    lines.sort_by_key(|l| l.time_ms.unwrap_or(u64::MAX));
    lines
}

fn parse_lrc_timestamp(tag: &str) -> Option<u64> {
    let (m_str, s_part) = tag.split_once(':')?;
    let mins: u64 = m_str.trim().parse().ok()?;
    let (s_str, ms_str) = match s_part.split_once('.') {
        Some((s, ms)) => (s, ms),
        None => (s_part, "0"),
    };
    let secs: u64 = s_str.trim().parse().ok()?;
    let ms_clean: String = ms_str.chars().take(3).collect();
    let ms_val: u64 = ms_clean.parse().unwrap_or(0);
    let ms = match ms_clean.len() {
        1 => ms_val * 100,
        2 => ms_val * 10,
        _ => ms_val,
    };
    Some(mins * 60_000 + secs * 1_000 + ms)
}

fn parse_plain(plain: &str) -> Vec<LyricsLine> {
    plain
        .lines()
        .map(|l| l.trim())
        .filter(|l| !l.is_empty())
        .map(|l| LyricsLine {
            time_ms: None,
            text: l.to_string(),
        })
        .collect()
}

pub const DEFAULT_SPICY_LYRICS_KEY: &str = "sl_sk_jUt9n2OY-tACouA9yP9A7_34f3nYYrfbbtsr60WJpro";

async fn fetch_spicy_lyrics(
    client: &Client,
    api_key: &str,
    track: &Track,
    primary_artist: &str,
) -> Option<LyricsResponse> {
    let mut target_id: Option<String> = None;

    if target_id.is_none() {
        let search_resp = client
            .get("https://api.spicylyrics.org/v1/tracks/search")
            .bearer_auth(api_key)
            .query(&[("title", track.title.as_str()), ("artist", primary_artist)])
            .send()
            .await
            .ok()?;
        if search_resp.status().is_success() {
            if let Ok(json) = search_resp.json::<Value>().await {
                target_id = json["Body"]["id"]
                    .as_str()
                    .or_else(|| json["id"].as_str())
                    .or_else(|| json["tracks"][0]["id"].as_str())
                    .or_else(|| json[0]["id"].as_str())
                    .map(|s| s.to_string());
            }
        }
    }

    let id = target_id?;
    let resp = client
        .get(format!("https://api.spicylyrics.org/v1/lyrics/{}", id))
        .bearer_auth(api_key)
        .send()
        .await
        .ok()?;
    if !resp.status().is_success() {
        return None;
    }
    let json = resp.json::<Value>().await.ok()?;
    let content = json["Body"]["Content"].as_array()?;
    let mut lines = Vec::new();
    for item in content {
        let start_sec = item["StartTime"].as_f64();
        let text = if let Some(syllables) = item["Syllables"].as_array() {
            syllables
                .iter()
                .filter_map(|s| s["Text"].as_str())
                .collect::<Vec<_>>()
                .join(" ")
        } else {
            item["Text"].as_str().unwrap_or("").to_string()
        };
        let clean = text.trim().to_string();
        if !clean.is_empty() {
            lines.push(LyricsLine {
                time_ms: start_sec.map(|s| (s * 1000.0) as u64),
                text: clean,
            });
        }
    }
    if lines.is_empty() {
        None
    } else {
        let synced = lines.iter().any(|l| l.time_ms.is_some());
        Some(LyricsResponse {
            synced,
            source: "Spicy Lyrics".into(),
            lines,
        })
    }
}

pub async fn fetch_lyrics(
    ym_client: &Client,
    ym_token: Option<&str>,
    spicy_key: Option<&str>,
    track: &Track,
) -> LyricsResponse {
    let doh = crate::network::proxy::shared_doh();
    let lrc_client = Client::builder()
        .connect_timeout(std::time::Duration::from_secs(4))
        .timeout(std::time::Duration::from_secs(6))
        .dns_resolver(doh.clone())
        .user_agent("FlowlyMusicPlayer/0.1.0 (https://github.com/flowly)")
        .build()
        .unwrap_or_else(|_| Client::new());

    let primary_artist = track
        .artist
        .split(&[',', '&'][..])
        .next()
        .unwrap_or(&track.artist)
        .trim()
        .to_string();

    // 0. Spicy Lyrics API (с коротким таймаутом 3.5с, чтобы не вешать загрузку текста)
    if let Some(key) = spicy_key.map(str::trim).filter(|k| !k.is_empty()) {
        let spicy_client = Client::builder()
            .connect_timeout(std::time::Duration::from_secs(4))
            .timeout(std::time::Duration::from_secs(5))
            .dns_resolver(doh)
            .user_agent("FlowlyMusicPlayer/0.1.0")
            .build()
            .unwrap_or_else(|_| lrc_client.clone());
        if let Some(spicy) = fetch_spicy_lyrics(&spicy_client, key, track, &primary_artist).await {
            return spicy;
        }
    }

    // 1. LRCLIB exact match
    if let Ok(resp) = lrc_client
        .get("https://lrclib.net/api/get")
        .query(&[
            ("artist_name", primary_artist.as_str()),
            ("track_name", track.title.as_str()),
            ("album_name", track.album.as_str()),
            ("duration", &track.duration.to_string()),
        ])
        .send()
        .await
    {
        if resp.status().is_success() {
            if let Ok(json) = resp.json::<Value>().await {
                if let Some(synced) = json["syncedLyrics"].as_str().filter(|s| !s.trim().is_empty()) {
                    let lines = parse_lrc(synced);
                    if !lines.is_empty() {
                        return LyricsResponse {
                            synced: true,
                            source: "LRCLIB".into(),
                            lines,
                        };
                    }
                }
                if let Some(plain) = json["plainLyrics"].as_str().filter(|s| !s.trim().is_empty()) {
                    let lines = parse_plain(plain);
                    if !lines.is_empty() {
                        return LyricsResponse {
                            synced: false,
                            source: "LRCLIB".into(),
                            lines,
                        };
                    }
                }
            }
        }
    }

    // 2. LRCLIB search fallback
    let search_q = format!("{} {}", primary_artist, track.title);
    if let Ok(resp) = lrc_client
        .get("https://lrclib.net/api/search")
        .query(&[("q", search_q.as_str())])
        .send()
        .await
    {
        if resp.status().is_success() {
            if let Ok(arr) = resp.json::<Vec<Value>>().await {
                if let Some(best_synced) = arr
                    .iter()
                    .find(|item| item["syncedLyrics"].as_str().map(|s| !s.trim().is_empty()).unwrap_or(false))
                {
                    if let Some(s) = best_synced["syncedLyrics"].as_str() {
                        let lines = parse_lrc(s);
                        if !lines.is_empty() {
                            return LyricsResponse {
                                synced: true,
                                source: "LRCLIB".into(),
                                lines,
                            };
                        }
                    }
                }
                if let Some(best_plain) = arr
                    .iter()
                    .find(|item| item["plainLyrics"].as_str().map(|s| !s.trim().is_empty()).unwrap_or(false))
                {
                    if let Some(p) = best_plain["plainLyrics"].as_str() {
                        let lines = parse_plain(p);
                        if !lines.is_empty() {
                            return LyricsResponse {
                                synced: false,
                                source: "LRCLIB".into(),
                                lines,
                            };
                        }
                    }
                }
            }
        }
    }

    // 3. Yandex Music supplement fallback
    if track.source == SourceKind::Yandex {
        if let Some(tok) = ym_token {
            let raw_id = track.id.strip_prefix("ym:").unwrap_or(&track.id);
            if let Ok(resp) = ym_client
                .get(format!("https://api.music.yandex.net/tracks/{}/supplement", raw_id))
                .header("Authorization", format!("OAuth {}", tok))
                .send()
                .await
            {
                if let Ok(json) = resp.json::<Value>().await {
                    if let Some(full) = json["result"]["lyrics"]["fullLyrics"]
                        .as_str()
                        .filter(|s| !s.trim().is_empty())
                    {
                        let lines = parse_plain(full);
                        if !lines.is_empty() {
                            return LyricsResponse {
                                synced: false,
                                source: "Yandex Music".into(),
                                lines,
                            };
                        }
                    }
                }
            }
        }
    }

    LyricsResponse {
        synced: false,
        source: "Не найдено".into(),
        lines: Vec::new(),
    }
}
