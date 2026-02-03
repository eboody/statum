#![feature(proc_macro_span)]

extern crate proc_macro;

use proc_macro::Span;
use proc_macro2::LineColumn;
use std::fs;
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
    let content = fs::read_to_string(file_path).ok()?;
    let parsed = syn::parse_file(&content).ok()?;

    let base = module_path_from_file(file_path);
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

/// Gets the full pseudo-absolute module path of the current macro invocation.
pub fn get_pseudo_module_path() -> String {
    get_source_info()
        .and_then(|(file, line)| find_module_path(&file, line))
        .unwrap_or_else(|| "unknown".to_string())
}
