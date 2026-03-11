use std::fs;
use std::time::UNIX_EPOCH;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct FileFingerprint {
    len: u64,
    modified_ns: Option<u128>,
}

#[derive(Clone)]
pub(crate) struct CachedValue<T> {
    pub(crate) fingerprint: FileFingerprint,
    pub(crate) value: T,
}

impl<T> CachedValue<T> {
    pub(crate) fn new(fingerprint: FileFingerprint, value: T) -> Self {
        Self { fingerprint, value }
    }
}

pub(crate) fn file_fingerprint(file_path: &str) -> Option<FileFingerprint> {
    let metadata = fs::metadata(file_path).ok()?;
    let modified_ns = metadata
        .modified()
        .ok()
        .and_then(|modified| modified.duration_since(UNIX_EPOCH).ok())
        .map(|duration| duration.as_nanos());
    Some(FileFingerprint {
        len: metadata.len(),
        modified_ns,
    })
}

pub(crate) fn fresh_cached_value<T: Clone>(
    cached: Option<CachedValue<T>>,
    fingerprint: FileFingerprint,
) -> Option<T> {
    let cached = cached?;
    if cached.fingerprint == fingerprint {
        Some(cached.value)
    } else {
        None
    }
}

pub(crate) fn tracked_file_matches(tracked_file_path: Option<&str>, file_path: &str) -> bool {
    tracked_file_path == Some(file_path)
}
