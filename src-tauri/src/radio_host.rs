use std::path::PathBuf;
use std::process::Stdio;
use std::time::{SystemTime, UNIX_EPOCH};

use base64::{engine::general_purpose::STANDARD as B64, Engine as _};
use futures_util::{SinkExt, StreamExt};
use rand::Rng;
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use tokio::io::AsyncWriteExt;
use tokio_tungstenite::{
    connect_async,
    tungstenite::{client::IntoClientRequest, Message},
};

use crate::models::Track;

const EDGE_TRUSTED_TOKEN: &str = "6A5AA1D4EAFF4E9FB37E23D68491D6F4";
const EDGE_GEC_VERSION: &str = "1-130.0.2849.68";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RadioHostResult {
    pub text: String,
    pub audio_data_url: Option<String>,
    pub used_engine: String,
    pub error: Option<String>,
}

#[derive(Debug, Clone)]
pub struct CachedWeather {
    pub city: String,
    pub summary: String,
    pub fetched_at: u64,
}

fn now_unix_sec() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

fn compute_sec_ms_gec() -> String {
    let unix = now_unix_sec();
    let mut ticks = unix + 11_644_473_600;
    ticks -= ticks % 300;
    let filetime_ticks = (ticks as u128) * 10_000_000;
    let raw = format!("{}{}", filetime_ticks, EDGE_TRUSTED_TOKEN);
    let mut hasher = Sha256::new();
    hasher.update(raw.as_bytes());
    format!("{:X}", hasher.finalize())
}

fn weather_code_ru(code: i64) -> &'static str {
    match code {
        0 => "ясно",
        1 | 2 => "переменная облачность",
        3 => "пасмурно",
        45 | 48 => "туман",
        51..=57 => "морось",
        61..=67 | 80..=82 => "дождь",
        71..=77 | 85 | 86 => "снег",
        95..=99 => "гроза",
        _ => "спокойная погода",
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WeatherAtmosphere {
    pub city: String,
    pub temp_c: i64,
    pub weather_kind: String, // clear | cloudy | rain | snow | storm | fog
    pub summary: String,
}

fn weather_kind_from_code(code: i64) -> &'static str {
    match code {
        0 | 1 => "clear",
        2 | 3 => "cloudy",
        45 | 48 => "fog",
        51..=67 | 80..=82 => "rain",
        71..=77 | 85 | 86 => "snow",
        95..=99 => "storm",
        _ => "cloudy",
    }
}

pub async fn fetch_weather_atmosphere(city: &str) -> Option<WeatherAtmosphere> {
    let clean = city.trim();
    if clean.is_empty() {
        return None;
    }
    let client = Client::builder()
        .timeout(std::time::Duration::from_secs(5))
        .build()
        .ok()?;

    let geo: Value = client
        .get("https://geocoding-api.open-meteo.com/v1/search")
        .query(&[
            ("name", clean),
            ("count", "1"),
            ("language", "ru"),
            ("format", "json"),
        ])
        .send()
        .await
        .ok()?
        .json()
        .await
        .ok()?;

    let first = geo["results"].as_array()?.first()?;
    let lat = first["latitude"].as_f64()?;
    let lon = first["longitude"].as_f64()?;
    let resolved_name = first["name"].as_str().unwrap_or(clean).to_string();

    let meteo: Value = client
        .get("https://api.open-meteo.com/v1/forecast")
        .query(&[
            ("latitude", lat.to_string()),
            ("longitude", lon.to_string()),
            ("current", "temperature_2m,weather_code".to_string()),
            ("timezone", "auto".to_string()),
        ])
        .send()
        .await
        .ok()?
        .json()
        .await
        .ok()?;

    let temp = meteo["current"]["temperature_2m"].as_f64()?.round() as i64;
    let code = meteo["current"]["weather_code"].as_i64().unwrap_or(0);
    let temp_str = if temp > 0 {
        format!("плюс {}", temp)
    } else if temp < 0 {
        format!("минус {}", temp.abs())
    } else {
        "около нуля".to_string()
    };

    Some(WeatherAtmosphere {
        city: resolved_name,
        temp_c: temp,
        weather_kind: weather_kind_from_code(code).to_string(),
        summary: format!("{} и {}", temp_str, weather_code_ru(code)),
    })
}

pub async fn fetch_weather_summary(city: &str) -> Option<String> {
    fetch_weather_atmosphere(city).await.map(|a| a.summary)
}

pub fn build_radio_phrase(
    track: &Track,
    local_hour: u8,
    weather: Option<&str>,
    is_wave: bool,
) -> String {
    let primary_artist = track
        .artist
        .split(&[',', '&'][..])
        .next()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .unwrap_or(&track.artist);

    let genre_low = track.genre.as_deref().unwrap_or("").to_lowercase();

    let mut candidates: Vec<String> = vec![
        format!(
            "А сейчас у нас заявка от тебя самого — слушаем {}, трек «{}».",
            primary_artist, track.title
        ),
        format!(
            "В эфире Flowly — {} с композицией «{}». Не переключайся!",
            primary_artist, track.title
        ),
        format!(
            "Дальше по курсу — {}, «{}». Делай погромче.",
            primary_artist, track.title
        ),
        format!(
            "Прямо сейчас в наушники залетает {} и трек «{}». Погнали!",
            primary_artist, track.title
        ),
        format!(
            "Небольшая пауза на вдох... и дальше звучит {} — «{}».",
            primary_artist, track.title
        ),
        format!(
            "Эфир продолжается. На очереди {}, «{}» — отличный выбор для этого момента.",
            primary_artist, track.title
        ),
        format!(
            "Если ты искал знак, чтобы отложить дела на пару минут — вот он. Играет {}, «{}».",
            primary_artist, track.title
        ),
        format!(
            "Переключаем передачу: следующий трек в нашем сете — {} и «{}».",
            primary_artist, track.title
        ),
        format!(
            "Без лишних предисловий — {}, «{}». Наслаждайся звуком.",
            primary_artist, track.title
        ),
    ];

    let has_any = |keys: &[&str]| keys.iter().any(|k| genre_low.contains(k));
    if has_any(&["rap", "hip", "trap", "drill", "phonk"]) {
        candidates.push(format!(
            "Качаем дальше: плотный бит от {} — «{}».",
            primary_artist, track.title
        ));
        candidates.push(format!(
            "Басы на максимум, в эфире {} с треком «{}».",
            primary_artist, track.title
        ));
    } else if has_any(&["rock", "alt", "punk", "metal", "indie"]) {
        candidates.push(format!(
            "Гитары накручены, настроение на месте — слушаем {}, «{}».",
            primary_artist, track.title
        ));
    } else if has_any(&["electr", "house", "techno", "edm", "dnb", "synth"]) {
        candidates.push(format!(
            "Ловим электронный пульс: за пультом сегодня {} — «{}».",
            primary_artist, track.title
        ));
    } else if has_any(&["lofi", "lo-fi", "ambient", "chill", "jazz"]) {
        candidates.push(format!(
            "Время чуть выдохнуть и поймать чилл. Для тебя играет {} — «{}».",
            primary_artist, track.title
        ));
    }

    if is_wave {
        candidates.push(format!(
            "Волна подбросила отличный вариант: {}, «{}».",
            primary_artist, track.title
        ));
        candidates.push(format!(
            "Алгоритм волны уверен, что это попадёт прямо в точку — {}, «{}».",
            primary_artist, track.title
        ));
    }

    if local_hour >= 23 || local_hour < 5 {
        candidates.push(format!(
            "На часах уже глубокая ночь, ночной эфир Flowly продолжается. Играет {} — «{}».",
            primary_artist, track.title
        ));
        candidates.push(format!(
            "Всем полуночникам привет! В ночной подборке звучит {} — «{}».",
            primary_artist, track.title
        ));
    } else if local_hour < 11 {
        candidates.push(format!(
            "Доброе утро! Заряжаемся кофе и треком от {} — «{}».",
            primary_artist, track.title
        ));
    } else if local_hour >= 18 {
        candidates.push(format!(
            "Вечерний эфир в самом разгаре. Включаем {} — «{}».",
            primary_artist, track.title
        ));
    }

    if let Some(w) = weather {
        candidates.push(format!(
            "За окном сейчас {}, а у нас в наушниках — {}, «{}».",
            w, primary_artist, track.title
        ));
        candidates.push(format!(
            "На улице {}, так что делаем звук чуть теплее. В эфире {} — «{}».",
            w, primary_artist, track.title
        ));
    }

    let idx = rand::thread_rng().gen_range(0..candidates.len());
    candidates.swap_remove(idx)
}

fn escape_xml(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

pub async fn synthesize_edge_tts(text: &str, voice_gender: &str) -> Result<String, String> {
    let voice_name = if voice_gender == "female" {
        "ru-RU-SvetlanaNeural"
    } else {
        "ru-RU-DmitryNeural"
    };

    let gec = compute_sec_ms_gec();
    let url = format!(
        "wss://speech.platform.bing.com/consumer/speech/synthesize/readaloud/edge/v1?TrustedClientToken={}&Sec-MS-GEC={}&Sec-MS-GEC-Version={}",
        EDGE_TRUSTED_TOKEN, gec, EDGE_GEC_VERSION
    );

    let mut req = url.into_client_request().map_err(|e| e.to_string())?;
    let headers = req.headers_mut();
    headers.insert(
        "User-Agent",
        "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/130.0.0.0 Safari/537.36 Edg/130.0.0.0"
            .parse()
            .unwrap(),
    );
    headers.insert(
        "Origin",
        "chrome-extension://jdiccldimpdaibmpdkjnbmckianbfold"
            .parse()
            .unwrap(),
    );

    let (mut ws, _) = tokio::time::timeout(std::time::Duration::from_secs(7), connect_async(req))
        .await
        .map_err(|_| "Таймаут подключения к Edge TTS".to_string())?
        .map_err(|e| e.to_string())?;

    let cfg_msg = "Content-Type:application/json; charset=utf-8\r\nPath:speech.config\r\n\r\n{\"context\":{\"synthesis\":{\"audio\":{\"metadataoptions\":{\"sentenceBoundaryEnabled\":false,\"wordBoundaryEnabled\":false},\"outputFormat\":\"audio-24khz-48kbitrate-mono-mp3\"}}}}";
    ws.send(Message::Text(cfg_msg.to_string().into()))
        .await
        .map_err(|e| e.to_string())?;

    let req_id = format!("{:032x}", rand::thread_rng().gen::<u128>());
    let pitch = if voice_gender == "female" { "+2Hz" } else { "-3Hz" };
    let ssml = format!(
        "<speak version='1.0' xmlns='http://www.w3.org/2001/10/synthesis' xml:lang='ru-RU'><voice name='{}'><prosody pitch='{}' rate='+9%' volume='+0%'>{}</prosody></voice></speak>",
        voice_name,
        pitch,
        escape_xml(text)
    );
    let ssml_msg = format!(
        "X-RequestId:{}\r\nContent-Type:application/ssml+xml\r\nPath:ssml\r\n\r\n{}",
        req_id, ssml
    );
    ws.send(Message::Text(ssml_msg.into()))
        .await
        .map_err(|e| e.to_string())?;

    let mut mp3_bytes: Vec<u8> = Vec::new();
    while let Some(Ok(msg)) = ws.next().await {
        match msg {
            Message::Binary(bin) => {
                if bin.len() >= 2 {
                    let hdr_len = u16::from_be_bytes([bin[0], bin[1]]) as usize;
                    let start = 2 + hdr_len;
                    if start <= bin.len() {
                        let hdr = String::from_utf8_lossy(&bin[2..start]);
                        if hdr.contains("Path:audio") {
                            mp3_bytes.extend_from_slice(&bin[start..]);
                        }
                    }
                }
            }
            Message::Text(txt) => {
                if txt.contains("Path:turn.end") {
                    break;
                }
            }
            Message::Close(_) => break,
            _ => {}
        }
    }

    if mp3_bytes.is_empty() {
        return Err("Пустой аудиопоток Edge TTS".to_string());
    }
    Ok(format!("data:audio/mpeg;base64,{}", B64.encode(&mp3_bytes)))
}

pub async fn synthesize_piper_tts(
    app_data_dir: &PathBuf,
    text: &str,
    voice_gender: &str,
) -> Result<String, String> {
    let piper_dir = app_data_dir.join("piper");
    let exe_name = if cfg!(windows) { "piper.exe" } else { "piper" };
    let mut piper_bin = piper_dir.join(exe_name);
    if !piper_bin.exists() {
        let nested = piper_dir.join("piper").join(exe_name);
        if nested.exists() {
            piper_bin = nested;
        }
    }
    if !piper_bin.exists() {
        return Err(format!(
            "Не найден Piper: нажмите «Установить Piper» или поместите {} и .onnx в {}",
            exe_name,
            piper_dir.display()
        ));
    }

    let model_name = if voice_gender == "female" {
        "ru_RU-irina-medium.onnx"
    } else {
        "ru_RU-dmitri-medium.onnx"
    };
    let mut model_path = piper_dir.join(model_name);
    if !model_path.exists() {
        // Ищем любую .onnx модель в папке piper/
        if let Ok(entries) = std::fs::read_dir(&piper_dir) {
            for ent in entries.flatten() {
                if ent.path().extension().and_then(|s| s.to_str()) == Some("onnx") {
                    model_path = ent.path();
                    break;
                }
            }
        }
    }
    if !model_path.exists() {
        return Err(format!("Не найдена .onnx модель в {}", piper_dir.display()));
    }

    let work_dir = piper_bin
        .parent()
        .map(|p| p.to_path_buf())
        .unwrap_or_else(|| piper_dir.clone());
    let model_rel = if work_dir == piper_dir {
        model_path
            .file_name()
            .map(|n| PathBuf::from(n))
            .unwrap_or_else(|| model_path.clone())
    } else {
        PathBuf::from("..").join(
            model_path
                .file_name()
                .unwrap_or_else(|| std::ffi::OsStr::new("ru_RU-dmitri-medium.onnx")),
        )
    };
    let out_name = format!("flowly_radio_{}.wav", now_unix_sec());
    let out_wav = work_dir.join(&out_name);

    let mut cmd = tokio::process::Command::new(&piper_bin);
    cmd.current_dir(&work_dir)
        .arg("--model")
        .arg(&model_rel)
        .arg("--espeak_data")
        .arg("espeak-ng-data")
        .arg("--output_file")
        .arg(&out_name)
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    #[cfg(windows)]
    {
        cmd.creation_flags(0x08000000); // CREATE_NO_WINDOW
    }

    let mut child = cmd.spawn().map_err(|e| e.to_string())?;

    if let Some(mut stdin) = child.stdin.take() {
        let _ = stdin.write_all(text.as_bytes()).await;
        let _ = stdin.shutdown().await;
    }

    let status = child.wait().await.map_err(|e| e.to_string())?;
    if !status.success() {
        return Err("Piper завершился с ошибкой".to_string());
    }

    let wav_bytes = tokio::fs::read(&out_wav).await.map_err(|e| e.to_string())?;
    let _ = tokio::fs::remove_file(&out_wav).await;
    Ok(format!("data:audio/wav;base64,{}", B64.encode(&wav_bytes)))
}

pub async fn synthesize_google_tts(text: &str) -> Result<String, String> {
    let client = Client::builder()
        .timeout(std::time::Duration::from_secs(7))
        .build()
        .map_err(|e| e.to_string())?;
    let resp = client
        .get("https://translate.google.com/translate_tts")
        .query(&[
            ("ie", "UTF-8"),
            ("tl", "ru"),
            ("client", "tw-ob"),
            ("q", text),
        ])
        .header(
            "User-Agent",
            "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36",
        )
        .send()
        .await
        .map_err(|e| e.to_string())?
        .error_for_status()
        .map_err(|e| e.to_string())?;
    let bytes = resp.bytes().await.map_err(|e| e.to_string())?;
    if bytes.is_empty() {
        return Err("Пустой ответ TTS".to_string());
    }
    Ok(format!("data:audio/mpeg;base64,{}", B64.encode(&bytes)))
}

pub async fn install_piper_bundle(app_data_dir: &PathBuf) -> Result<String, String> {
    let piper_dir = app_data_dir.join("piper");
    tokio::fs::create_dir_all(&piper_dir)
        .await
        .map_err(|e| e.to_string())?;

    let client = Client::builder()
        .timeout(std::time::Duration::from_secs(180))
        .build()
        .map_err(|e| e.to_string())?;

    // 1. Скачиваем и распаковываем piper_windows_amd64.zip (если ещё нет)
    let has_exe = piper_dir.join("piper.exe").exists()
        || piper_dir.join("piper").join("piper.exe").exists();
    if !has_exe {
        let zip_url = "https://github.com/rhasspy/piper/releases/download/2023.11.14-2/piper_windows_amd64.zip";
        let resp = client
            .get(zip_url)
            .send()
            .await
            .map_err(|e| format!("Ошибка загрузки Piper: {}", e))?
            .error_for_status()
            .map_err(|e| format!("Ошибка загрузки Piper: {}", e))?;
        let bytes = resp.bytes().await.map_err(|e| e.to_string())?;
        let zip_path = piper_dir.join("piper_windows_amd64.zip");
        tokio::fs::write(&zip_path, &bytes)
            .await
            .map_err(|e| e.to_string())?;

        let status = tokio::process::Command::new("powershell")
            .args([
                "-NoProfile",
                "-Command",
                &format!(
                    "Expand-Archive -Path '{}' -DestinationPath '{}' -Force",
                    zip_path.display(),
                    piper_dir.display()
                ),
            ])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .await
            .map_err(|e| format!("Ошибка распаковки Piper: {}", e))?;
        let _ = tokio::fs::remove_file(&zip_path).await;
        if !status.success() {
            return Err("Не удалось распаковать архив Piper".to_string());
        }
    }

    // 2. Скачиваем русские нейросетевые модели (Дмитрий и Ирина)
    let files = [
        (
            "ru_RU-dmitri-medium.onnx",
            "https://huggingface.co/rhasspy/piper-voices/resolve/v1.0.0/ru/ru_RU/dmitri/medium/ru_RU-dmitri-medium.onnx",
        ),
        (
            "ru_RU-dmitri-medium.onnx.json",
            "https://huggingface.co/rhasspy/piper-voices/resolve/v1.0.0/ru/ru_RU/dmitri/medium/ru_RU-dmitri-medium.onnx.json",
        ),
        (
            "ru_RU-irina-medium.onnx",
            "https://huggingface.co/rhasspy/piper-voices/resolve/v1.0.0/ru/ru_RU/irina/medium/ru_RU-irina-medium.onnx",
        ),
        (
            "ru_RU-irina-medium.onnx.json",
            "https://huggingface.co/rhasspy/piper-voices/resolve/v1.0.0/ru/ru_RU/irina/medium/ru_RU-irina-medium.onnx.json",
        ),
    ];

    for (name, url) in files {
        let dest = piper_dir.join(name);
        if dest.exists() {
            continue;
        }
        let resp = client
            .get(url)
            .send()
            .await
            .map_err(|e| format!("Ошибка загрузки {}: {}", name, e))?
            .error_for_status()
            .map_err(|e| format!("Ошибка загрузки {}: {}", name, e))?;
        let bytes = resp.bytes().await.map_err(|e| e.to_string())?;
        tokio::fs::write(&dest, &bytes)
            .await
            .map_err(|e| e.to_string())?;
    }

    Ok("Piper и русские нейро-голоса (Дмитрий и Ирина) установлены!".to_string())
}
