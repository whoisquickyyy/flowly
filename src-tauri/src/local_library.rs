use std::path::{Path, PathBuf};

use base64::Engine;
use lofty::file::{AudioFile, TaggedFileExt};
use lofty::tag::Accessor;

use crate::models::{SourceKind, Track};

const DEFAULT_LOCAL_ARTWORK: &str = "data:image/svg+xml;utf8,<svg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 120 120'><rect width='120' height='120' rx='18' fill='%23181822'/><circle cx='60' cy='60' r='34' fill='none' stroke='%23a855f7' stroke-width='4' opacity='0.7'/><circle cx='60' cy='60' r='10' fill='%23a855f7'/></svg>";

fn is_audio_ext(path: &Path) -> bool {
    let Some(ext) = path.extension().and_then(|e| e.to_str()) else {
        return false;
    };
    matches!(
        ext.to_ascii_lowercase().as_str(),
        "mp3" | "flac" | "wav" | "ogg" | "m4a" | "aac" | "opus" | "webm"
    )
}

fn collect_audio_files(dir: &Path, depth: u8, out: &mut Vec<PathBuf>) {
    if depth > 8 || out.len() >= 5000 {
        return;
    }
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_audio_files(&path, depth + 1, out);
        } else if is_audio_ext(&path) {
            out.push(path);
        }
    }
}

fn hash_path(s: &str) -> u64 {
    let mut h: u64 = 14695981039346656037;
    for b in s.as_bytes() {
        h ^= *b as u64;
        h = h.wrapping_mul(1099511628211);
    }
    h
}

fn parse_filename_fallback(path: &Path) -> (String, String) {
    let stem = path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("Без названия")
        .trim();
    if let Some((a, t)) = stem.split_once(" - ") {
        let artist = a.trim();
        let title = t.trim();
        if !artist.is_empty() && !title.is_empty() {
            return (artist.to_string(), title.to_string());
        }
    }
    ("Локальный трек".to_string(), stem.to_string())
}

pub fn scan_local_folders(folders: &[String]) -> Vec<Track> {
    let mut files = Vec::new();
    for folder in folders {
        let p = Path::new(folder);
        if p.exists() && p.is_dir() {
            collect_audio_files(p, 0, &mut files);
        }
    }

    let mut seen = std::collections::HashSet::new();
    let mut tracks = Vec::with_capacity(files.len());

    for path in files {
        let path_str = path.to_string_lossy().to_string();
        if !seen.insert(path_str.clone()) {
            continue;
        }
        let (fallback_artist, fallback_title) = parse_filename_fallback(&path);
        let mut title = fallback_title;
        let mut artist = fallback_artist;
        let mut album = path
            .parent()
            .and_then(|p| p.file_name())
            .and_then(|n| n.to_str())
            .unwrap_or("Локальная папка")
            .to_string();
        let mut genre: Option<String> = None;
        let mut duration: u32 = 0;
        let mut release_year: Option<u16> = None;
        let mut artwork = DEFAULT_LOCAL_ARTWORK.to_string();

        if let Ok(tagged) = lofty::read_from_path(&path) {
            duration = tagged.properties().duration().as_secs() as u32;
            if let Some(tag) = tagged.primary_tag().or_else(|| tagged.first_tag()) {
                if let Some(t) = tag.title().filter(|s| !s.trim().is_empty()) {
                    title = t.trim().to_string();
                }
                if let Some(a) = tag.artist().filter(|s| !s.trim().is_empty()) {
                    artist = a.trim().to_string();
                }
                if let Some(al) = tag.album().filter(|s| !s.trim().is_empty()) {
                    album = al.trim().to_string();
                }
                if let Some(g) = tag.genre().filter(|s| !s.trim().is_empty()) {
                    genre = Some(g.trim().to_string());
                }
                if let Some(y) = tag.year() {
                    if y > 1900 && y < 2100 {
                        release_year = Some(y as u16);
                    }
                }
                if let Some(pic) = tag.pictures().first() {
                    let bytes = pic.data();
                    if !bytes.is_empty() && bytes.len() <= 256 * 1024 {
                        let mime = pic
                            .mime_type()
                            .map(|m| m.as_str())
                            .unwrap_or("image/jpeg");
                        let b64 = base64::engine::general_purpose::STANDARD.encode(bytes);
                        artwork = format!("data:{};base64,{}", mime, b64);
                    }
                }
            }
        }

        tracks.push(Track {
            id: format!("local:{:016x}", hash_path(&path_str)),
            title,
            artist,
            album,
            artwork,
            duration,
            source: SourceKind::Local,
            genre,
            release_year,
            playback_url: Some(format!("local://{}", path_str)),
            external_url: String::new(),
            audio_quality: Some("Local".to_string()),
            rating: None,
            accent_color: "#a855f7".to_string(),
        });
    }

    tracks
}
