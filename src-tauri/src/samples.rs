use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TrackSample {
    pub title: String,
    pub artist: String,
    pub relation: String,
    pub mbid: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TrackSamplesResponse {
    pub samples: Vec<TrackSample>,
    pub whosampled_url: String,
}

fn primary_artist(artist: &str) -> &str {
    artist
        .split(&[',', '&'][..])
        .next()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .unwrap_or(artist)
}

fn clean_title(title: &str) -> String {
    // Убираем скобки вроде (feat. ...) или [Remastered] для более точного поиска в MusicBrainz
    let mut out = String::with_capacity(title.len());
    let mut depth = 0u32;
    for ch in title.chars() {
        match ch {
            '(' | '[' => depth += 1,
            ')' | ']' => {
                if depth > 0 {
                    depth -= 1;
                }
            }
            _ if depth == 0 => out.push(ch),
            _ => {}
        }
    }
    out.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn extract_artist_credit(val: &Value) -> String {
    val["artist-credit"]
        .as_array()
        .map(|arr| {
            let mut s = String::new();
            for item in arr {
                if let Some(name) = item["name"].as_str().or_else(|| item["artist"]["name"].as_str()) {
                    s.push_str(name);
                }
                if let Some(joinphrase) = item["joinphrase"].as_str() {
                    s.push_str(joinphrase);
                }
            }
            s.trim().to_string()
        })
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "Unknown Artist".to_string())
}

pub async fn fetch_track_samples(artist: &str, title: &str) -> TrackSamplesResponse {
    let p_artist = primary_artist(artist);
    let c_title = clean_title(title);
    let ws_query = url::form_urlencoded::byte_serialize(format!("{} {}", p_artist, c_title).as_bytes())
        .collect::<String>();
    let whosampled_url = format!("https://www.whosampled.com/search/?q={}", ws_query);

    let client = Client::builder()
        .user_agent("FlowlyPlayer/0.1.0 (https://github.com/flowly)")
        .timeout(std::time::Duration::from_secs(8))
        .build()
        .unwrap_or_else(|_| Client::new());

    let query = format!("recording:\"{}\" AND artist:\"{}\"", c_title, p_artist);
    let Ok(search_resp) = client
        .get("https://musicbrainz.org/ws/2/recording/")
        .query(&[("query", query.as_str()), ("limit", "3"), ("fmt", "json")])
        .send()
        .await
    else {
        return TrackSamplesResponse {
            samples: Vec::new(),
            whosampled_url,
        };
    };

    let Ok(search_json) = search_resp.json::<Value>().await else {
        return TrackSamplesResponse {
            samples: Vec::new(),
            whosampled_url,
        };
    };

    let mbids: Vec<String> = search_json["recordings"]
        .as_array()
        .map(|arr| {
            arr.iter()
                .filter_map(|r| r["id"].as_str().map(String::from))
                .take(2)
                .collect()
        })
        .unwrap_or_default();

    let mut samples: Vec<TrackSample> = Vec::new();
    let mut seen_mbids = std::collections::HashSet::new();

    for mbid in mbids {
        let url = format!("https://musicbrainz.org/ws/2/recording/{}", mbid);
        let Ok(rec_resp) = client
            .get(&url)
            .query(&[("inc", "recording-rels+artist-credits"), ("fmt", "json")])
            .send()
            .await
        else {
            continue;
        };
        let Ok(rec_json) = rec_resp.json::<Value>().await else {
            continue;
        };

        if let Some(rels) = rec_json["relations"].as_array() {
            for rel in rels {
                let rel_type = rel["type"].as_str().unwrap_or("");
                let direction = rel["direction"].as_str().unwrap_or("");
                // forward: текущий трек сэмплирует / интерполирует target recording
                let is_sample_rel = matches!(
                    rel_type,
                    "samples" | "interpolates" | "mashup" | "medley" | "quoted"
                );
                if !is_sample_rel || direction == "backward" {
                    continue;
                }
                let rec = &rel["recording"];
                let Some(target_id) = rec["id"].as_str() else {
                    continue;
                };
                if !seen_mbids.insert(target_id.to_string()) {
                    continue;
                }
                let Some(target_title) = rec["title"].as_str() else {
                    continue;
                };
                let target_artist = extract_artist_credit(rec);
                let relation_label = match rel_type {
                    "samples" => "Сэмпл",
                    "interpolates" => "Интерполяция",
                    "mashup" => "Мэшап",
                    "medley" => "Попурри",
                    _ => "Цитирование",
                }
                .to_string();

                samples.push(TrackSample {
                    title: target_title.to_string(),
                    artist: target_artist,
                    relation: relation_label,
                    mbid: target_id.to_string(),
                });
            }
        }
        if !samples.is_empty() {
            break;
        }
    }

    TrackSamplesResponse {
        samples,
        whosampled_url,
    }
}
