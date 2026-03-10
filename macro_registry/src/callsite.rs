use module_path_extractor::{find_module_path, get_pseudo_module_path, get_source_info};
use std::path::Path;

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

/// Returns `(file_path, line_number)` for the current macro call-site when available.
pub fn current_source_info() -> Option<(String, usize)> {
    get_source_info().map(|(file_path, line_number)| (normalize_file_path(&file_path), line_number))
}

/// Returns the best-effort module path for the current macro call-site.
pub fn current_module_path() -> String {
    get_pseudo_module_path()
}

/// Resolves the module path for a specific source file and line number.
pub fn module_path_for_line(file_path: &str, line_number: usize) -> Option<String> {
    find_module_path(file_path, line_number)
}
