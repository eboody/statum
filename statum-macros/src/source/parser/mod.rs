use std::fs;
use std::path::Path;

mod line_map;
mod module_ranges;

use super::pathing::module_path_from_file_with_root;
use line_map::build_line_module_paths;

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) enum LineModulePath {
    Unset,
    Exact { path: String, depth: usize },
    Ambiguous,
}

fn compose_module_path(base: &str, nested: &str) -> String {
    if base == "crate" {
        nested.to_string()
    } else {
        format!("{base}::{nested}")
    }
}

pub(crate) fn resolve_module_path_from_lines(
    base_module: &str,
    line_modules: &[LineModulePath],
    line_number: usize,
) -> Option<String> {
    if line_number == 0 {
        return Some(base_module.to_string());
    }

    match line_modules.get(line_number - 1) {
        Some(LineModulePath::Exact { path, .. }) => Some(compose_module_path(base_module, path)),
        Some(LineModulePath::Unset) | None => Some(base_module.to_string()),
        Some(LineModulePath::Ambiguous) => None,
    }
}

pub(crate) fn parse_file_modules(
    file_path: &str,
    module_root: &Path,
) -> Option<(String, Vec<LineModulePath>)> {
    let content = fs::read_to_string(file_path).ok()?;
    let base_module = module_path_from_file_with_root(file_path, module_root);
    let line_modules = build_line_module_paths(&content)?;
    Some((base_module, line_modules))
}
