extern crate proc_macro;

mod cache;
mod parser;
mod pathing;

#[cfg(doctest)]
#[doc = include_str!("../README.md")]
mod readme_doctests {}

use std::path::Path;

use crate::cache::{
    clear_line_cache_for_file, file_fingerprint, get_or_parse_file_modules, store_line_result,
};
use parser::{parse_file_modules, resolve_module_path_from_lines};
use pathing::normalize_file_path;
pub use pathing::{
    module_path_from_file, module_path_from_file_with_root, module_path_to_file,
    module_root_from_file,
};

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

    match cache::cached_line_result(&normalized_file_path, line_number, fingerprint) {
        cache::CacheLookup::Fresh(module_path) => return module_path,
        // Cached line/module mappings are only valid as a set for one file fingerprint.
        // Once the file changes, drop every cached line for that file before reparsing.
        cache::CacheLookup::Stale => clear_line_cache_for_file(&normalized_file_path),
        cache::CacheLookup::Missing => {}
    }

    let parsed_file = get_or_parse_file_modules(&normalized_file_path, fingerprint)?;
    let resolved = resolve_module_path_from_lines(
        &parsed_file.base_module,
        &parsed_file.line_modules,
        line_number,
    );

    store_line_result(
        &normalized_file_path,
        line_number,
        fingerprint,
        resolved.clone(),
    );

    resolved
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

/// Gets the full pseudo-absolute module path of the current macro invocation.
pub fn get_pseudo_module_path() -> String {
    get_source_info()
        .and_then(|(file, line)| find_module_path(&file, line))
        .unwrap_or_else(|| "unknown".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::{Path, PathBuf};
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
    fn find_module_path_in_file_handles_raw_identifier_modules() {
        let crate_dir = unique_temp_dir("raw_ident_mods");
        let src = crate_dir.join("src");
        let lib = src.join("lib.rs");

        write_file(
            &lib,
            "#[cfg(any())]\npub(crate) mod r#async {\n    pub mod r#type {\n        pub fn marker() {}\n    }\n}\n",
        );

        let found = find_module_path_in_file(&lib.to_string_lossy(), 4, &src);
        assert_eq!(found.as_deref(), Some("r#async::r#type"));

        let _ = fs::remove_dir_all(crate_dir);
    }

    #[test]
    fn find_module_path_in_file_separates_sibling_modules_with_similar_shapes() {
        let crate_dir = unique_temp_dir("sibling_modules");
        let src = crate_dir.join("src");
        let lib = src.join("lib.rs");

        write_file(
            &lib,
            "mod alpha {\n    mod support {\n        pub struct Text;\n    }\n\n    pub enum WorkflowState {\n        Draft,\n    }\n\n    pub struct Row {\n        pub status: &'static str,\n    }\n}\n\nmod beta {\n    mod support {\n        pub struct Text;\n    }\n\n    pub enum WorkflowState {\n        Draft,\n    }\n\n    pub struct Row {\n        pub status: &'static str,\n    }\n}\n",
        );

        assert_eq!(
            find_module_path_in_file(&lib.to_string_lossy(), 6, &src).as_deref(),
            Some("alpha")
        );
        assert_eq!(
            find_module_path_in_file(&lib.to_string_lossy(), 20, &src).as_deref(),
            Some("beta")
        );
        assert_eq!(
            find_module_path_in_file(&lib.to_string_lossy(), 19, &src).as_deref(),
            Some("beta")
        );

        let _ = fs::remove_dir_all(crate_dir);
    }

    #[test]
    fn find_module_path_in_file_ignores_mod_tokens_in_comments_and_raw_strings() {
        let crate_dir = unique_temp_dir("comments_and_raw_strings");
        let src = crate_dir.join("src");
        let lib = src.join("lib.rs");

        write_file(
            &lib,
            "const TEMPLATE: &str = r#\"\nmod fake {\n    mod nested {}\n}\n\"#;\n\n/* mod ignored {\n    mod deeper {}\n} */\n\nmod outer {\n    // mod hidden { mod nope {} }\n    mod inner {\n        pub fn marker() {}\n    }\n}\n",
        );

        let found = find_module_path_in_file(&lib.to_string_lossy(), 14, &src);
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
        assert_eq!(cache::line_cache_entries_for(&lib_path), 2);

        // Ensure the file metadata timestamp has a chance to advance on coarse filesystems.
        thread::sleep(Duration::from_millis(2));
        write_file(
            &lib,
            "mod changed {\n    mod deeper {\n        pub fn marker() {}\n    }\n}\n",
        );

        let refreshed = find_module_path(&lib_path, 3);
        assert_eq!(refreshed.as_deref(), Some("changed::deeper"));
        assert_eq!(cache::line_cache_entries_for(&lib_path), 1);

        let second_line = find_module_path(&lib_path, 2);
        assert_eq!(second_line.as_deref(), Some("changed::deeper"));
        assert_eq!(cache::line_cache_entries_for(&lib_path), 2);

        let _ = fs::remove_dir_all(crate_dir);
    }
}
