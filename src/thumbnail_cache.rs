use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

use sha2::{Digest, Sha256};

const INDEX_FILE: &str = "index.json";
const DEFAULT_MAX_AGE_SECS: u64 = 30 * 24 * 60 * 60;

#[derive(Default, serde::Deserialize, serde::Serialize)]
struct Index {
    entries: HashMap<String, Entry>,
}

#[derive(serde::Deserialize, serde::Serialize)]
struct Entry {
    url: String,
    file: String,
    cached_at: u64,
}

pub struct ThumbnailCache {
    dir: PathBuf,
    index: Mutex<Index>,
}

impl ThumbnailCache {
    pub fn open() -> Self {
        let mut dir = glib::user_cache_dir();
        dir.push("oceans-ink");
        dir.push("thumbnails");
        Self::open_at(dir)
    }

    fn open_at(dir: PathBuf) -> Self {
        let _ = std::fs::create_dir_all(&dir);
        let index = std::fs::read(dir.join(INDEX_FILE))
            .ok()
            .and_then(|data| serde_json::from_slice::<Index>(&data).ok())
            .unwrap_or_default();
        let cache = Self {
            dir,
            index: Mutex::new(index),
        };
        cache.prune(Self::now() - DEFAULT_MAX_AGE_SECS);
        cache
    }

    fn now() -> u64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|duration| duration.as_secs())
            .unwrap_or(0)
    }

    fn key(url: &str) -> String {
        let mut hasher = Sha256::new();
        hasher.update(url.as_bytes());
        hasher
            .finalize()
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect()
    }

    pub fn get(&self, url: &str) -> Option<Vec<u8>> {
        let index = Self::lock(&self.index);
        let entry = index.entries.get(url)?;
        std::fs::read(self.dir.join(&entry.file)).ok()
    }

    pub fn put(&self, url: &str, bytes: &[u8]) {
        let file = format!("{}.img", Self::key(url));
        if std::fs::write(self.dir.join(&file), bytes).is_err() {
            return;
        }
        let mut index = Self::lock(&self.index);
        index.entries.insert(
            url.to_string(),
            Entry {
                url: url.to_string(),
                file,
                cached_at: Self::now(),
            },
        );
        Self::persist(&self.dir, &index);
    }

    /// Removes cached files written before `cutoff` (seconds since the Unix epoch).
    pub fn prune(&self, cutoff: u64) {
        let mut removed: Vec<String> = Vec::new();
        {
            let mut index = Self::lock(&self.index);
            index.entries.retain(|_, entry| {
                if entry.cached_at < cutoff {
                    removed.push(entry.file.clone());
                    false
                } else {
                    true
                }
            });
            Self::persist(&self.dir, &index);
        }
        for file in removed {
            let _ = std::fs::remove_file(self.dir.join(file));
        }
    }

    fn lock(index: &Mutex<Index>) -> std::sync::MutexGuard<'_, Index> {
        index
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    fn persist(dir: &std::path::Path, index: &Index) {
        let Ok(data) = serde_json::to_vec(index) else {
            return;
        };
        let temp = dir.join(format!("{INDEX_FILE}.tmp"));
        if std::fs::write(&temp, data).is_ok() {
            let _ = std::fs::rename(&temp, dir.join(INDEX_FILE));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir(name: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("oceans-ink-test-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        dir
    }

    #[test]
    fn round_trips_bytes_by_url() {
        let cache = ThumbnailCache::open_at(temp_dir("round-trip"));
        assert_eq!(cache.get("https://example.com/a.jpg"), None);
        cache.put("https://example.com/a.jpg", b"image-bytes");
        assert_eq!(
            cache.get("https://example.com/a.jpg"),
            Some(b"image-bytes".to_vec())
        );
    }

    #[test]
    fn persists_across_instances() {
        let dir = temp_dir("persist");
        {
            let cache = ThumbnailCache::open_at(dir.clone());
            cache.put("https://example.com/b.jpg", b"payload");
        }
        let reopened = ThumbnailCache::open_at(dir);
        assert_eq!(
            reopened.get("https://example.com/b.jpg"),
            Some(b"payload".to_vec())
        );
    }

    #[test]
    fn prunes_entries_older_than_cutoff() {
        let dir = temp_dir("prune");
        let cache = ThumbnailCache::open_at(dir);
        cache.put("https://example.com/old.jpg", b"old");
        cache.put("https://example.com/new.jpg", b"new");

        let cutoff = ThumbnailCache::now() - DEFAULT_MAX_AGE_SECS;
        {
            let mut index = cache.index.lock().unwrap();
            let entry = index
                .entries
                .get_mut("https://example.com/old.jpg")
                .unwrap();
            entry.cached_at = cutoff - 10;
        }

        cache.prune(cutoff);

        assert_eq!(cache.get("https://example.com/old.jpg"), None);
        assert_eq!(
            cache.get("https://example.com/new.jpg"),
            Some(b"new".to_vec())
        );
    }

    #[test]
    fn prunes_on_open() {
        let dir = temp_dir("prune-on-open");
        let now = ThumbnailCache::now();
        {
            let cache = ThumbnailCache::open_at(dir.clone());
            cache.put("https://example.com/stale.jpg", b"stale");
            let mut index = cache.index.lock().unwrap();
            let entry = index
                .entries
                .get_mut("https://example.com/stale.jpg")
                .unwrap();
            entry.cached_at = now - DEFAULT_MAX_AGE_SECS - 10;
            ThumbnailCache::persist(&dir, &index);
        }
        let reopened = ThumbnailCache::open_at(dir);
        assert_eq!(reopened.get("https://example.com/stale.jpg"), None);
    }
}
