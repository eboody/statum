extern crate proc_macro;

use proc_macro2::LineColumn;
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{OnceLock, RwLock};
use syn::spanned::Spanned;
use syn::Item;

type ModulePathCache = HashMap<(String, usize), Option<String>>;

static MODULE_PATH_CACHE: OnceLock<RwLock<ModulePathCache>> = OnceLock::new();

fn get_module_path_cache() -> &'static RwLock<ModulePathCache> {
    MODULE_PATH_CACHE.get_or_init(|| RwLock::new(HashMap::new()))
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
    let cache_key = (file_path.to_string(), line_number);
    if let Some(cached) = get_module_path_cache()
        .read()
        .ok()
        .and_then(|cache| cache.get(&cache_key).cloned())
    {
        return cached;
    }

    let module_root = module_root_from_file(file_path);
    let resolved = find_module_path_in_file(file_path, line_number, &module_root);

    if let Ok(mut cache) = get_module_path_cache().write() {
        cache.insert(cache_key, resolved.clone());
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
    let content = fs::read_to_string(file_path).ok()?;
    let parsed = syn::parse_file(&content).ok()?;

    let base = module_path_from_file_with_root(file_path, module_root);
    let mut best_stack: Vec<String> = Vec::new();

    fn span_contains_line(span: proc_macro2::Span, line: usize) -> bool {
        let LineColumn { line: start, .. } = span.start();
        let LineColumn { line: end, .. } = span.end();
        line >= start && line <= end
    }

    fn visit_items(items: &[Item], line: usize, stack: &mut Vec<String>, best: &mut Vec<String>) {
        for item in items {
            let Item::Mod(module) = item else { continue };
            let Some((_, inner_items)) = &module.content else {
                continue;
            };
            if !span_contains_line(module.span(), line) {
                continue;
            }

            stack.push(module.ident.to_string());
            if stack.len() > best.len() {
                *best = stack.clone();
            }
            visit_items(inner_items, line, stack, best);
            stack.pop();
        }
    }

    visit_items(&parsed.items, line_number, &mut Vec::new(), &mut best_stack);

    if best_stack.is_empty() {
        return Some(base);
    }

    let nested = best_stack.join("::");
    if base == "crate" {
        Some(nested)
    } else {
        Some(format!("{base}::{nested}"))
    }
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
}
