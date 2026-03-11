use std::path::{Path, PathBuf};

pub(crate) fn normalize_file_path(file_path: &str) -> String {
    let path = Path::new(file_path);
    if path.is_absolute() {
        return path.to_string_lossy().into_owned();
    }

    match std::env::current_dir() {
        Ok(cwd) => cwd.join(path).to_string_lossy().into_owned(),
        Err(_) => file_path.to_string(),
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
