use module_path_extractor::{find_module_path, get_pseudo_module_path, get_source_info};

/// Returns `(file_path, line_number)` for the current macro call-site when available.
pub fn current_source_info() -> Option<(String, usize)> {
    get_source_info()
}

/// Returns the best-effort module path for the current macro call-site.
pub fn current_module_path() -> String {
    get_pseudo_module_path()
}

/// Resolves the module path for a specific source file and line number.
pub fn module_path_for_line(file_path: &str, line_number: usize) -> Option<String> {
    find_module_path(file_path, line_number)
}
