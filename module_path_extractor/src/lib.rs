extern crate proc_macro;

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{OnceLock, RwLock};
use std::time::UNIX_EPOCH;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct FileFingerprint {
    len: u64,
    modified_ns: Option<u128>,
}

#[derive(Clone, Debug)]
struct ParsedFileModules {
    fingerprint: FileFingerprint,
    base_module: String,
    line_modules: Vec<String>,
}

#[derive(Clone, Debug)]
struct CachedLineResult {
    fingerprint: FileFingerprint,
    module_path: Option<String>,
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

fn clear_line_cache_for_file(file_path: &str) {
    if let Ok(mut cache) = get_line_result_cache().write() {
        cache.retain(|(cached_path, _), _| cached_path != file_path);
    }
}

fn file_fingerprint(file_path: &str) -> Option<FileFingerprint> {
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

fn normalize_file_path(file_path: &str) -> String {
    let path = Path::new(file_path);
    if path.is_absolute() {
        return path.to_string_lossy().into_owned();
    }

    match std::env::current_dir() {
        Ok(cwd) => cwd.join(path).to_string_lossy().into_owned(),
        Err(_) => file_path.to_string(),
    }
}

fn compose_module_path(base: &str, nested: &str) -> String {
    if base == "crate" {
        nested.to_string()
    } else {
        format!("{base}::{nested}")
    }
}

fn resolve_module_path_from_lines(
    base_module: &str,
    line_modules: &[String],
    line_number: usize,
) -> Option<String> {
    if line_number == 0 {
        return Some(base_module.to_string());
    }

    match line_modules.get(line_number - 1) {
        Some(path) if !path.is_empty() => Some(compose_module_path(base_module, path)),
        _ => Some(base_module.to_string()),
    }
}

fn is_ident_start(byte: u8) -> bool {
    byte == b'_' || byte.is_ascii_alphabetic()
}

fn is_ident_continue(byte: u8) -> bool {
    is_ident_start(byte) || byte.is_ascii_digit()
}

fn raw_string_prefix_len(bytes: &[u8], start: usize) -> Option<(usize, usize)> {
    if bytes.get(start) != Some(&b'r') {
        return None;
    }
    let mut idx = start + 1;
    let mut hashes = 0usize;
    while bytes.get(idx) == Some(&b'#') {
        hashes += 1;
        idx += 1;
    }
    if bytes.get(idx) != Some(&b'"') {
        return None;
    }
    Some((hashes, idx - start + 1))
}

fn build_line_module_paths(content: &str) -> Vec<String> {
    #[derive(Clone, Copy)]
    enum Mode {
        Normal,
        LineComment,
        BlockComment { depth: usize },
        String { escaped: bool },
        Char { escaped: bool },
        RawString { hashes: usize },
    }

    let bytes = content.as_bytes();
    let mut line_paths = vec![String::new()];
    let mut line = 1usize;
    let mut i = 0usize;
    let mut mode = Mode::Normal;
    let mut brace_stack: Vec<Option<String>> = Vec::new();
    let mut module_stack: Vec<String> = Vec::new();
    let mut expect_mod_ident = false;
    let mut pending_mod_name: Option<String> = None;
    let mut expect_mod_open = false;

    let current_module_path = |stack: &[String]| -> String {
        if stack.is_empty() {
            String::new()
        } else {
            stack.join("::")
        }
    };

    while i < bytes.len() {
        let byte = bytes[i];

        if byte == b'\n' {
            line += 1;
            if line_paths.len() < line {
                line_paths.push(current_module_path(&module_stack));
            } else if let Some(existing) = line_paths.get_mut(line - 1) {
                *existing = current_module_path(&module_stack);
            }
        }

        match mode {
            Mode::LineComment => {
                if byte == b'\n' {
                    mode = Mode::Normal;
                }
                i += 1;
                continue;
            }
            Mode::BlockComment { depth } => {
                if byte == b'/' && bytes.get(i + 1) == Some(&b'*') {
                    mode = Mode::BlockComment { depth: depth + 1 };
                    i += 2;
                    continue;
                }
                if byte == b'*' && bytes.get(i + 1) == Some(&b'/') {
                    if depth == 1 {
                        mode = Mode::Normal;
                    } else {
                        mode = Mode::BlockComment { depth: depth - 1 };
                    }
                    i += 2;
                    continue;
                }
                i += 1;
                continue;
            }
            Mode::String { escaped } => {
                if byte == b'\\' && !escaped {
                    mode = Mode::String { escaped: true };
                } else if byte == b'"' && !escaped {
                    mode = Mode::Normal;
                } else {
                    mode = Mode::String { escaped: false };
                }
                i += 1;
                continue;
            }
            Mode::Char { escaped } => {
                if byte == b'\\' && !escaped {
                    mode = Mode::Char { escaped: true };
                } else if byte == b'\'' && !escaped {
                    mode = Mode::Normal;
                } else {
                    mode = Mode::Char { escaped: false };
                }
                i += 1;
                continue;
            }
            Mode::RawString { hashes } => {
                if byte == b'"' {
                    let mut matched = true;
                    for offset in 0..hashes {
                        if bytes.get(i + 1 + offset) != Some(&b'#') {
                            matched = false;
                            break;
                        }
                    }
                    if matched {
                        mode = Mode::Normal;
                        i += 1 + hashes;
                        continue;
                    }
                }
                i += 1;
                continue;
            }
            Mode::Normal => {}
        }

        if byte == b'/' && bytes.get(i + 1) == Some(&b'/') {
            mode = Mode::LineComment;
            i += 2;
            continue;
        }
        if byte == b'/' && bytes.get(i + 1) == Some(&b'*') {
            mode = Mode::BlockComment { depth: 1 };
            i += 2;
            continue;
        }
        if byte == b'"' {
            mode = Mode::String { escaped: false };
            i += 1;
            continue;
        }
        if byte == b'\'' {
            mode = Mode::Char { escaped: false };
            i += 1;
            continue;
        }
        if let Some((hashes, consumed)) = raw_string_prefix_len(bytes, i) {
            mode = Mode::RawString { hashes };
            i += consumed;
            continue;
        }

        if is_ident_start(byte) {
            let start = i;
            i += 1;
            while i < bytes.len() && is_ident_continue(bytes[i]) {
                i += 1;
            }
            let token = &content[start..i];

            if expect_mod_ident {
                pending_mod_name = Some(token.to_string());
                expect_mod_ident = false;
                expect_mod_open = true;
                continue;
            }

            if token == "mod" {
                expect_mod_ident = true;
                pending_mod_name = None;
                expect_mod_open = false;
            } else if expect_mod_open {
                pending_mod_name = None;
                expect_mod_open = false;
            }
            continue;
        }

        if byte == b'{' {
            if let Some(module_name) = pending_mod_name.take() {
                module_stack.push(module_name.clone());
                brace_stack.push(Some(module_name));
                if let Some(current) = line_paths.get_mut(line - 1) {
                    *current = current_module_path(&module_stack);
                }
            } else {
                brace_stack.push(None);
            }
            expect_mod_ident = false;
            expect_mod_open = false;
            i += 1;
            continue;
        }

        if byte == b'}' {
            if let Some(marker) = brace_stack.pop() {
                if marker.is_some() {
                    let _ = module_stack.pop();
                    if let Some(current) = line_paths.get_mut(line - 1) {
                        *current = current_module_path(&module_stack);
                    }
                }
            }
            expect_mod_ident = false;
            expect_mod_open = false;
            i += 1;
            continue;
        }

        if byte == b';' {
            pending_mod_name = None;
            expect_mod_ident = false;
            expect_mod_open = false;
            i += 1;
            continue;
        }

        i += 1;
    }

    line_paths
}

fn parse_file_modules(file_path: &str, module_root: &Path) -> Option<(String, Vec<String>)> {
    let content = fs::read_to_string(file_path).ok()?;
    let base_module = module_path_from_file_with_root(file_path, module_root);
    let line_modules = build_line_module_paths(&content);
    Some((base_module, line_modules))
}

fn get_or_parse_file_modules(
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

/// Extracts the file path and line number where the macro was invoked.
pub fn get_source_info() -> Option<(String, usize)> {
    // `proc_macro` APIs panic when used outside a proc-macro context.
    // Return `None` instead of panicking so callers can degrade gracefully.
    let span = std::panic::catch_unwind(proc_macro::Span::call_site).ok()?;
    let line_number = span.start().line();

    if let Some(local_file) = span.local_file() {
        return Some((local_file.to_string_lossy().into_owned(), line_number));
    }

    let file_path = span.file();
    if file_path.is_empty() {
        None
    } else {
        Some((file_path, line_number))
    }
}

/// Reads the file and extracts the module path at the given line.
pub fn find_module_path(file_path: &str, line_number: usize) -> Option<String> {
    let normalized_file_path = normalize_file_path(file_path);
    let fingerprint = file_fingerprint(&normalized_file_path)?;
    let cache_key = (normalized_file_path.clone(), line_number);
    let mut stale_cache_for_file = false;
    if let Some(cached) = get_line_result_cache()
        .read()
        .ok()
        .and_then(|cache| cache.get(&cache_key).cloned())
    {
        if cached.fingerprint == fingerprint {
            return cached.module_path;
        }
        stale_cache_for_file = true;
    }

    if stale_cache_for_file {
        clear_line_cache_for_file(&normalized_file_path);
    }

    let parsed_file = get_or_parse_file_modules(&normalized_file_path, fingerprint)?;
    let resolved = resolve_module_path_from_lines(
        &parsed_file.base_module,
        &parsed_file.line_modules,
        line_number,
    );

    if let Ok(mut cache) = get_line_result_cache().write() {
        cache.insert(
            cache_key,
            CachedLineResult {
                fingerprint,
                module_path: resolved.clone(),
            },
        );
    }

    resolved
}

/// Converts a file path into a pseudo-Rust module path.
pub fn module_path_from_file(file_path: &str) -> String {
    let normalized = file_path.replace('\\', "/");
    let relative = normalized
        .split_once("/src/")
        .map(|(_, tail)| tail)
        .unwrap_or(normalized.as_str());

    if relative == "lib.rs" || relative == "main.rs" {
        return "crate".to_string();
    }

    let without_ext = relative.strip_suffix(".rs").unwrap_or(relative);
    if without_ext.ends_with("/mod") {
        let parent = without_ext.strip_suffix("/mod").unwrap_or(without_ext);
        let parent = parent.trim_matches('/');
        return parent.replace('/', "::");
    }

    let module = without_ext.trim_matches('/').replace('/', "::");
    if module.is_empty() {
        "crate".to_string()
    } else {
        module
    }
}

/// Derives a module root (usually `<crate>/src`) from a file path.
pub fn module_root_from_file(file_path: &str) -> PathBuf {
    let normalized = file_path.replace('\\', "/");
    if let Some((root, _)) = normalized.rsplit_once("/src/") {
        let mut root = PathBuf::from(root);
        root.push("src");
        return root;
    }

    Path::new(file_path)
        .parent()
        .map(|p| p.to_path_buf())
        .unwrap_or_else(|| PathBuf::from("."))
}

/// Converts a file path into a pseudo-Rust module path using a known module root.
pub fn module_path_from_file_with_root(file_path: &str, module_root: &Path) -> String {
    let normalized = file_path.replace('\\', "/");
    let module_root_norm = module_root.to_string_lossy().replace('\\', "/");

    let relative = match normalized.strip_prefix(&(module_root_norm.clone() + "/")) {
        Some(rel) => rel,
        None => return module_path_from_file(file_path),
    };

    if relative == "lib.rs" || relative == "main.rs" {
        return "crate".to_string();
    }

    let without_ext = relative.strip_suffix(".rs").unwrap_or(relative);
    if without_ext.ends_with("/mod") {
        let parent = without_ext.strip_suffix("/mod").unwrap_or(without_ext);
        let parent = parent.trim_matches('/');
        return parent.replace('/', "::");
    }

    let module = without_ext.trim_matches('/').replace('/', "::");
    if module.is_empty() {
        "crate".to_string()
    } else {
        module
    }
}

/// Reads the file and extracts the module path at the given line, using a known module root.
pub fn find_module_path_in_file(
    file_path: &str,
    line_number: usize,
    module_root: &Path,
) -> Option<String> {
    let normalized_file_path = normalize_file_path(file_path);
    let (base_module, line_modules) = parse_file_modules(&normalized_file_path, module_root)?;
    resolve_module_path_from_lines(&base_module, &line_modules, line_number)
}

/// Maps a module path (e.g. `crate::foo::bar`) to a source file.
pub fn module_path_to_file(
    module_path: &str,
    current_file: &str,
    module_root: &Path,
) -> Option<PathBuf> {
    let module_path = module_path.strip_prefix("crate::").unwrap_or(module_path);
    if module_path == "crate" || module_path.is_empty() {
        let lib = module_root.join("lib.rs");
        if lib.exists() {
            return Some(lib);
        }
        let main = module_root.join("main.rs");
        if main.exists() {
            return Some(main);
        }
        let current = PathBuf::from(current_file);
        if current.exists() {
            return Some(current);
        }
        return None;
    }

    let rel = module_path.replace("::", "/");
    let candidate = module_root.join(format!("{rel}.rs"));
    if candidate.exists() {
        return Some(candidate);
    }
    let candidate = module_root.join(rel).join("mod.rs");
    if candidate.exists() {
        return Some(candidate);
    }
    None
}

/// Gets the full pseudo-absolute module path of the current macro invocation.
pub fn get_pseudo_module_path() -> String {
    get_source_info()
        .and_then(|(file, line)| find_module_path(&file, line))
        .unwrap_or_else(|| "unknown".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread;
    use std::time::Duration;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn unique_temp_dir(label: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock")
            .as_nanos();
        let dir = std::env::temp_dir().join(format!("statum_module_path_{label}_{nanos}"));
        fs::create_dir_all(&dir).expect("create temp dir");
        dir
    }

    fn write_file(path: &Path, contents: &str) {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).expect("create parent");
        }
        fs::write(path, contents).expect("write file");
    }

    fn line_cache_entries_for(file_path: &str) -> usize {
        let normalized = normalize_file_path(file_path);
        get_line_result_cache()
            .read()
            .expect("line cache lock")
            .keys()
            .filter(|(cached_path, _)| cached_path == &normalized)
            .count()
    }

    #[test]
    fn module_path_from_file_handles_lib_mod_and_nested_paths() {
        assert_eq!(module_path_from_file("/tmp/project/src/lib.rs"), "crate");
        assert_eq!(module_path_from_file("/tmp/project/src/main.rs"), "crate");
        assert_eq!(
            module_path_from_file("/tmp/project/src/foo/bar.rs"),
            "foo::bar"
        );
        assert_eq!(module_path_from_file("/tmp/project/src/foo/mod.rs"), "foo");
    }

    #[test]
    fn module_path_to_file_resolves_crate_rs_and_mod_rs() {
        let crate_dir = unique_temp_dir("to_file");
        let src = crate_dir.join("src");
        let lib = src.join("lib.rs");
        let workflow = src.join("workflow.rs");
        let worker_mod = src.join("worker").join("mod.rs");

        write_file(&lib, "pub mod workflow; pub mod worker;");
        write_file(&workflow, "pub fn run() {}");
        write_file(&worker_mod, "pub fn spawn() {}");

        let current = workflow.to_string_lossy().into_owned();
        let module_root = src;

        assert_eq!(
            module_path_to_file("crate", &current, &module_root),
            Some(lib.clone())
        );
        assert_eq!(
            module_path_to_file("crate::workflow", &current, &module_root),
            Some(workflow.clone())
        );
        assert_eq!(
            module_path_to_file("crate::worker", &current, &module_root),
            Some(worker_mod.clone())
        );

        let _ = fs::remove_dir_all(crate_dir);
    }

    #[test]
    fn find_module_path_in_file_resolves_nested_inline_modules() {
        let crate_dir = unique_temp_dir("nested_mods");
        let src = crate_dir.join("src");
        let lib = src.join("lib.rs");

        write_file(
            &lib,
            "mod outer {\n    mod inner {\n        pub fn marker() {}\n    }\n}\n",
        );

        let found = find_module_path_in_file(&lib.to_string_lossy(), 3, &src);
        assert_eq!(found.as_deref(), Some("outer::inner"));

        let _ = fs::remove_dir_all(crate_dir);
    }

    #[test]
    fn find_module_path_invalidates_stale_line_cache_when_file_changes() {
        let crate_dir = unique_temp_dir("invalidate_cache");
        let src = crate_dir.join("src");
        let lib = src.join("lib.rs");

        write_file(
            &lib,
            "mod outer {\n    mod inner {\n        pub fn marker() {}\n    }\n}\n",
        );

        let lib_path = lib.to_string_lossy().to_string();
        let first = find_module_path(&lib_path, 3);
        assert_eq!(first.as_deref(), Some("outer::inner"));

        // Ensure the file metadata timestamp has a chance to advance on coarse filesystems.
        thread::sleep(Duration::from_millis(2));
        write_file(
            &lib,
            "mod changed {\n    mod deeper {\n        pub fn marker() {}\n    }\n}\n",
        );

        let second = find_module_path(&lib_path, 3);
        assert_eq!(second.as_deref(), Some("changed::deeper"));

        let _ = fs::remove_dir_all(crate_dir);
    }

    #[test]
    fn stale_line_entries_are_replaced_after_file_change() {
        let crate_dir = unique_temp_dir("stale_line_entries");
        let src = crate_dir.join("src");
        let lib = src.join("lib.rs");

        write_file(
            &lib,
            "mod outer {\n    mod inner {\n        pub fn marker() {}\n    }\n}\n",
        );

        let lib_path = lib.to_string_lossy().to_string();
        let _ = find_module_path(&lib_path, 2);
        let _ = find_module_path(&lib_path, 3);
        assert_eq!(line_cache_entries_for(&lib_path), 2);

        // Ensure the file metadata timestamp has a chance to advance on coarse filesystems.
        thread::sleep(Duration::from_millis(2));
        write_file(
            &lib,
            "mod changed {\n    mod deeper {\n        pub fn marker() {}\n    }\n}\n",
        );

        let refreshed = find_module_path(&lib_path, 3);
        assert_eq!(refreshed.as_deref(), Some("changed::deeper"));
        assert_eq!(line_cache_entries_for(&lib_path), 1);

        let second_line = find_module_path(&lib_path, 2);
        assert_eq!(second_line.as_deref(), Some("changed::deeper"));
        assert_eq!(line_cache_entries_for(&lib_path), 2);

        let _ = fs::remove_dir_all(crate_dir);
    }
}
