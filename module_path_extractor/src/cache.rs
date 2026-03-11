use std::collections::HashMap;
use std::fs;
use std::sync::{OnceLock, RwLock};
use std::time::UNIX_EPOCH;

use crate::parser::parse_file_modules;
use crate::pathing::module_root_from_file;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct FileFingerprint {
    len: u64,
    modified_ns: Option<u128>,
}

#[derive(Clone, Debug)]
pub(crate) struct ParsedFileModules {
    pub(crate) fingerprint: FileFingerprint,
    pub(crate) base_module: String,
    pub(crate) line_modules: Vec<String>,
}

#[derive(Clone, Debug)]
struct CachedLineResult {
    fingerprint: FileFingerprint,
    module_path: Option<String>,
}

pub(crate) enum CacheLookup<T> {
    Fresh(T),
    Stale,
    Missing,
}

type LineResultCache = HashMap<(String, usize), CachedLineResult>;
type FileModuleCache = HashMap<String, ParsedFileModules>;

static LINE_RESULT_CACHE: OnceLock<RwLock<LineResultCache>> = OnceLock::new();
static FILE_MODULE_CACHE: OnceLock<RwLock<FileModuleCache>> = OnceLock::new();

fn get_line_result_cache() -> &'static RwLock<LineResultCache> {
    LINE_RESULT_CACHE.get_or_init(|| RwLock::new(HashMap::new()))
}

fn get_file_module_cache() -> &'static RwLock<FileModuleCache> {
    FILE_MODULE_CACHE.get_or_init(|| RwLock::new(HashMap::new()))
}

pub(crate) fn clear_line_cache_for_file(file_path: &str) {
    if let Ok(mut cache) = get_line_result_cache().write() {
        cache.retain(|(cached_path, _), _| cached_path != file_path);
    }
}

pub(crate) fn cached_line_result(
    file_path: &str,
    line_number: usize,
    fingerprint: FileFingerprint,
) -> CacheLookup<Option<String>> {
    let cache_key = (file_path.to_string(), line_number);
    let Some(cached) = get_line_result_cache()
        .read()
        .ok()
        .and_then(|cache| cache.get(&cache_key).cloned())
    else {
        return CacheLookup::Missing;
    };

    if cached.fingerprint == fingerprint {
        CacheLookup::Fresh(cached.module_path)
    } else {
        CacheLookup::Stale
    }
}

pub(crate) fn store_line_result(
    file_path: &str,
    line_number: usize,
    fingerprint: FileFingerprint,
    module_path: Option<String>,
) {
    if let Ok(mut cache) = get_line_result_cache().write() {
        cache.insert(
            (file_path.to_string(), line_number),
            CachedLineResult {
                fingerprint,
                module_path,
            },
        );
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

pub(crate) fn get_or_parse_file_modules(
    file_path: &str,
    fingerprint: FileFingerprint,
) -> Option<ParsedFileModules> {
    if let Some(cached) = get_file_module_cache()
        .read()
        .ok()
        .and_then(|cache| cache.get(file_path).cloned())
    {
        if cached.fingerprint == fingerprint {
            return Some(cached);
        }
    }

    let module_root = module_root_from_file(file_path);
    let (base_module, line_modules) = parse_file_modules(file_path, &module_root)?;
    let parsed = ParsedFileModules {
        fingerprint,
        base_module,
        line_modules,
    };

    if let Ok(mut cache) = get_file_module_cache().write() {
        cache.insert(file_path.to_string(), parsed.clone());
    }

    Some(parsed)
}

#[cfg(test)]
pub(crate) fn line_cache_entries_for(file_path: &str) -> usize {
    let normalized = crate::pathing::normalize_file_path(file_path);
    get_line_result_cache()
        .read()
        .expect("line cache lock")
        .keys()
        .filter(|(cached_path, _)| cached_path == &normalized)
        .count()
}
