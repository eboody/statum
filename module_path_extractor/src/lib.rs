#![feature(proc_macro_span)]

extern crate proc_macro;

use proc_macro::Span;
use std::fs;

/// Extracts the file path and line number where the macro was invoked.
pub fn get_source_info() -> Option<(String, usize)> {
    let span = Span::call_site().source();
    let file_path = span.local_file()?;
    let file_path = file_path.to_string_lossy().to_string();
    let line_number = span.line();
    Some((file_path, line_number))
}

/// Reads the file and extracts the module path.
pub fn find_module_path(file_path: &str, line_number: usize) -> Option<String> {
    let content = fs::read_to_string(file_path).ok()?;

    let mut module_path = Vec::new();
    let mut last_inline_module: Option<String> = None;

    for (i, line) in content.lines().enumerate() {
        if i >= line_number {
            break;
        }

        let trimmed = line.trim();

        if let Some(mod_name) = trimmed.strip_prefix("mod ") {
            if let Some(mod_name) = mod_name.strip_suffix('{') {
                let mod_name = mod_name.trim().to_string();
                last_inline_module = Some(mod_name.clone());
                module_path.clear();
                module_path.push(mod_name);
            }
        } else if let Some(pub_mod_name) = trimmed.strip_prefix("pub mod ") {
            if let Some(pub_mod_name) = pub_mod_name.strip_suffix('{') {
                let pub_mod_name = pub_mod_name.trim().to_string();
                last_inline_module = Some(pub_mod_name.clone());
                module_path.clear();
                module_path.push(pub_mod_name);
            }
        }
    }

    if !module_path.is_empty() {
        return Some(module_path.join("::"));
    }

    Some(generate_pseudo_module_path(file_path, last_inline_module))
}

/// Converts a file path into a pseudo-Rust module path.
pub fn generate_pseudo_module_path(file_path: &str, inline_module: Option<String>) -> String {
    let filename = file_path.rsplit('/').next().unwrap_or(file_path);
    let filename = filename.split('.').next().unwrap_or(filename);

    let parent_dir = file_path.rsplit_once('/').map(|(dir, _)| dir).unwrap_or("");
    let parent_dir = parent_dir.replace("/", "::");
    let parent_dir = parent_dir.strip_prefix("src::").unwrap_or(&parent_dir);

    if filename == "main" || filename == "lib" {
        return "crate".to_string();
    }

    if let Some(inline_mod) = inline_module {
        return inline_mod;
    }

    if parent_dir.is_empty() {
        filename.to_string()
    } else {
        format!("{}::{}", parent_dir, filename)
    }
}

/// Gets the full pseudo-absolute module path of the current macro invocation.
pub fn get_pseudo_module_path() -> String {
    get_source_info()
        .and_then(|(file, line)| find_module_path(&file, line))
        .unwrap_or_else(|| "unknown".to_string())
}
