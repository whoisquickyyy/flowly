use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::SystemTime;

const MAX_CACHE_BYTES: u64 = 400 * 1024 * 1024; // 400 MB LRU cache

#[derive(Clone)]
pub struct TrackCache {
    dir: PathBuf,
    enabled: std::sync::Arc<AtomicBool>,
}

impl TrackCache {
    pub fn new(app_data_dir: &Path) -> Self {
        let dir = app_data_dir.join("track_cache");
        let _ = std::fs::create_dir_all(&dir);
        Self {
            dir,
            enabled: std::sync::Arc::new(AtomicBool::new(true)),
        }
    }

    pub fn set_enabled(&self, on: bool) {
        self.enabled.store(on, Ordering::Relaxed);
    }

    pub fn is_enabled(&self) -> bool {
        self.enabled.load(Ordering::Relaxed)
    }

    fn key_hash(url: &str) -> String {
        use sha2::{Digest, Sha256};
        let mut hasher = Sha256::new();
        hasher.update(url.as_bytes());
        let res = hasher.finalize();
        res.iter().take(16).map(|b| format!("{:02x}", b)).collect()
    }

    pub fn entry_paths(&self, cache_key: &str) -> (PathBuf, PathBuf) {
        let h = Self::key_hash(cache_key);
        (
            self.dir.join(format!("{}.bin", h)),
            self.dir.join(format!("{}.mime", h)),
        )
    }

    pub fn get_cached(&self, cache_key: &str) -> Option<(PathBuf, String)> {
        if !self.is_enabled() {
            return None;
        }
        let (bin, mime_file) = self.entry_paths(cache_key);
        if !bin.exists() {
            return None;
        }
        let meta = std::fs::metadata(&bin).ok()?;
        if meta.len() < 16 * 1024 {
            return None;
        }
        let mime = std::fs::read_to_string(&mime_file)
            .unwrap_or_else(|_| "audio/mpeg".to_string())
            .trim()
            .to_string();
        Some((bin, mime))
    }

    pub async fn store_bytes(&self, cache_key: &str, mime: &str, data: &[u8]) {
        if !self.is_enabled() || data.len() < 16 * 1024 || data.len() > 60 * 1024 * 1024 {
            return;
        }
        let (bin, mime_file) = self.entry_paths(cache_key);
        let tmp = bin.with_extension("tmp");
        if tokio::fs::write(&tmp, data).await.is_ok() {
            let _ = tokio::fs::write(&mime_file, mime.as_bytes()).await;
            let _ = tokio::fs::rename(&tmp, &bin).await;
            self.enforce_limit();
        }
    }

    fn enforce_limit(&self) {
        let Ok(entries) = std::fs::read_dir(&self.dir) else {
            return;
        };
        let mut files: Vec<(PathBuf, u64, SystemTime)> = Vec::new();
        let mut total: u64 = 0;
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) != Some("bin") {
                continue;
            }
            if let Ok(meta) = entry.metadata() {
                let sz = meta.len();
                total += sz;
                let mtime = meta.modified().unwrap_or(SystemTime::UNIX_EPOCH);
                files.push((path, sz, mtime));
            }
        }
        if total <= MAX_CACHE_BYTES {
            return;
        }
        files.sort_by_key(|(_, _, t)| *t);
        for (path, sz, _) in files {
            if total <= MAX_CACHE_BYTES * 85 / 100 {
                break;
            }
            let mime_path = path.with_extension("mime");
            let _ = std::fs::remove_file(&path);
            let _ = std::fs::remove_file(&mime_path);
            total = total.saturating_sub(sz);
        }
    }
}
