use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    sync::{Mutex, OnceLock},
    time::SystemTime,
};

/// Parsed JSONL cache for run timelines.
///
/// The UI polls the timeline every second while a run is active. Re-reading and
/// re-parsing the whole log on each poll makes a long run quadratic, so the
/// cache keeps the parsed events and reads only the bytes appended since the
/// previous call. Logs are append-only; any shrink or rewrite rebuilds.
#[derive(Default)]
struct LogCache {
    /// Byte offset after the last complete line already parsed.
    len: u64,
    modified: Option<SystemTime>,
    events: Vec<serde_json::Value>,
}

const LOG_CACHE_LIMIT: usize = 32;

fn log_cache() -> &'static Mutex<HashMap<PathBuf, LogCache>> {
    static CACHE: OnceLock<Mutex<HashMap<PathBuf, LogCache>>> = OnceLock::new();
    CACHE.get_or_init(|| Mutex::new(HashMap::new()))
}

pub(super) fn parse_lines(text: &str, into: &mut Vec<serde_json::Value>) {
    for line in text.lines() {
        if let Ok(value) = serde_json::from_str(line) {
            into.push(value);
        }
    }
}

pub(super) fn read_bytes_from(path: &Path, offset: u64) -> Option<Vec<u8>> {
    use std::io::{Read, Seek, SeekFrom};
    let mut file = std::fs::File::open(path).ok()?;
    if offset > 0 {
        file.seek(SeekFrom::Start(offset)).ok()?;
    }
    let mut bytes = Vec::new();
    file.read_to_end(&mut bytes).ok()?;
    Some(bytes)
}

/// Events of an append-only JSONL log, reading only what changed since the
/// previous call. Returns `None` when the file cannot be opened.
pub(super) fn cached_events(path: &Path) -> Option<Vec<serde_json::Value>> {
    let metadata = std::fs::metadata(path).ok()?;
    let len = metadata.len();
    let modified = metadata.modified().ok();
    let mut cache = log_cache()
        .lock()
        .unwrap_or_else(|poison| poison.into_inner());
    if let Some(entry) = cache.get(path)
        && entry.len == len
        && entry.modified == modified
    {
        return Some(entry.events.clone());
    }

    let previous = cache.remove(path);
    // Only trust the cache for a strict append. A rewrite with the same or a
    // larger size is not detectable from metadata alone.
    let (mut events, base) = match previous {
        Some(entry) if entry.len < len => (entry.events, entry.len),
        _ => (Vec::new(), 0),
    };
    let bytes = read_bytes_from(path, base)?;
    let text = String::from_utf8_lossy(&bytes);
    // A writer may still be in the middle of the last line; keep it for later.
    let complete = text.rfind('\n').map_or(0, |index| index + 1);
    parse_lines(&text[..complete], &mut events);
    let consumed = base + complete as u64;

    if cache.len() >= LOG_CACHE_LIMIT {
        cache.clear();
    }
    cache.insert(
        path.to_path_buf(),
        LogCache {
            len: consumed,
            modified,
            events: events.clone(),
        },
    );
    Some(events)
}
