#![feature(proc_macro_span)]

extern crate proc_macro;

use proc_macro::Span;
use proc_macro2::LineColumn;
use std::fs;
use std::path::{Path, PathBuf};
use syn::spanned::Spanned;
use syn::Item;

/// Extracts the file path and line number where the macro was invoked.
pub fn get_source_info() -> Option<(String, usize)> {
    let span = Span::call_site().source();
    let file_path = span.local_file()?;
    let file_path = file_path.to_string_lossy().to_string();
    let line_number = span.line();
    Some((file_path, line_number))
}

/// Reads the file and extracts the module path at the given line.
pub fn find_module_path(file_path: &str, line_number: usize) -> Option<String> {
    let module_root = module_root_from_file(file_path);
    find_module_path_in_file(file_path, line_number, &module_root)
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
        let parent = without_ext
            .strip_suffix("/mod")
            .unwrap_or(without_ext);
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
        let parent = without_ext
            .strip_suffix("/mod")
            .unwrap_or(without_ext);
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

    fn visit_items(
        items: &[Item],
        line: usize,
        stack: &mut Vec<String>,
        best: &mut Vec<String>,
    ) {
        for item in items {
            let Item::Mod(module) = item else { continue };
            let Some((_, inner_items)) = &module.content else { continue };
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
