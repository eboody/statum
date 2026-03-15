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
pub fn current_module_path_opt() -> Option<String> {
    let (file_path, line_number) = current_source_info()?;
    module_path_for_line(&file_path, line_number)
}

/// Returns the best-effort module path for the current macro call-site.
pub fn current_module_path() -> String {
    current_module_path_opt().unwrap_or_else(get_pseudo_module_path)
}

/// Resolves the module path for a specific source file and line number.
pub fn module_path_for_line(file_path: &str, line_number: usize) -> Option<String> {
    find_module_path(file_path, line_number)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::time::{SystemTime, UNIX_EPOCH};

    fn unique_temp_dir(label: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock")
            .as_nanos();
        let dir = std::env::temp_dir().join(format!("statum_callsite_{label}_{nanos}"));
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
    fn module_path_for_line_resolves_raw_identifier_modules() {
        let crate_dir = unique_temp_dir("raw_ident_mods");
        let src = crate_dir.join("src");
        let lib = src.join("lib.rs");

        write_file(
            &lib,
            "pub(crate) mod r#async {\n    pub mod r#type {\n        pub fn marker() {}\n    }\n}\n",
        );

        let found = module_path_for_line(&lib.to_string_lossy(), 3);
        assert_eq!(found.as_deref(), Some("r#async::r#type"));

        let _ = fs::remove_dir_all(crate_dir);
    }
}
