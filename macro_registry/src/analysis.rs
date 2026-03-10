use std::cell::RefCell;
use std::collections::HashMap;
use std::fs;
use std::rc::Rc;

/// Cached enum entry extracted from a parsed source file.
#[derive(Clone)]
pub struct EnumEntry {
    pub item: syn::ItemEnum,
    pub line_number: usize,
    pub attrs: Vec<String>,
}

/// Cached struct entry extracted from a parsed source file.
#[derive(Clone)]
pub struct StructEntry {
    pub item: syn::ItemStruct,
    pub line_number: usize,
    pub attrs: Vec<String>,
}

/// Parsed/cached representation of enums and structs in one source file.
#[derive(Clone, Default)]
pub struct FileAnalysis {
    pub enums: Vec<EnumEntry>,
    pub structs: Vec<StructEntry>,
}

thread_local! {
    static FILE_ANALYSIS_CACHE: RefCell<HashMap<String, Rc<FileAnalysis>>> = RefCell::new(HashMap::new());
}

/// Returns cached analysis for `file_path`, parsing and caching on first access.
pub fn get_file_analysis(file_path: &str) -> Option<Rc<FileAnalysis>> {
    if let Some(cached) = FILE_ANALYSIS_CACHE.with(|cache| cache.borrow().get(file_path).cloned()) {
        return Some(cached);
    }

    let analysis = Rc::new(build_file_analysis(file_path)?);
    FILE_ANALYSIS_CACHE.with(|cache| {
        cache
            .borrow_mut()
            .insert(file_path.to_string(), analysis.clone());
    });
    Some(analysis)
}

fn build_file_analysis(file_path: &str) -> Option<FileAnalysis> {
    let contents = fs::read_to_string(file_path).ok()?;
    let parsed = syn::parse_file(&contents).ok()?;
    let mut analysis = FileAnalysis::default();

    for item in parsed.items {
        match item {
            syn::Item::Enum(item_enum) => {
                let name = item_enum.ident.to_string();
                let span_line = item_enum.ident.span().start().line;
                let line_number = if span_line > 0 {
                    span_line
                } else {
                    find_item_line(&contents, "enum", &name)?
                };
                analysis.enums.push(EnumEntry {
                    attrs: attribute_names(&item_enum.attrs),
                    item: item_enum,
                    line_number,
                });
            }
            syn::Item::Struct(item_struct) => {
                let name = item_struct.ident.to_string();
                let span_line = item_struct.ident.span().start().line;
                let line_number = if span_line > 0 {
                    span_line
                } else {
                    find_item_line(&contents, "struct", &name)?
                };
                analysis.structs.push(StructEntry {
                    attrs: attribute_names(&item_struct.attrs),
                    item: item_struct,
                    line_number,
                });
            }
            _ => {}
        }
    }

    Some(analysis)
}

fn attribute_names(attrs: &[syn::Attribute]) -> Vec<String> {
    let mut names = Vec::new();

    for attr in attrs {
        let Some(ident) = attr.path().get_ident() else {
            continue;
        };
        let name = ident.to_string();
        if !names.iter().any(|existing| existing == &name) {
            names.push(name);
        }
    }

    names
}

fn find_item_line(contents: &str, kind: &str, item_name: &str) -> Option<usize> {
    let plain = format!("{kind} {item_name}");
    let pub_plain = format!("pub {kind} {item_name}");

    for (idx, line) in contents.lines().enumerate() {
        let trimmed = line.trim_start();
        if trimmed.starts_with(&plain) || trimmed.starts_with(&pub_plain) {
            return Some(idx + 1);
        }
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn write_temp_rust_file(contents: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock")
            .as_nanos();
        let path = std::env::temp_dir().join(format!("statum_analysis_{nanos}.rs"));
        fs::write(&path, contents).expect("write temp file");
        path
    }

    #[test]
    fn parses_line_numbers_for_struct_and_enum() {
        let path = write_temp_rust_file(
            r#"
#[state]
pub(crate) enum MyState {
    A,
}

#[machine]
pub struct MyMachine<MyState> {
    id: u64,
}
"#,
        );

        let analysis = build_file_analysis(path.to_str().expect("path")).expect("analysis");
        assert_eq!(analysis.enums.len(), 1);
        assert_eq!(analysis.structs.len(), 1);
        assert!(analysis.enums[0].line_number > 0);
        assert!(analysis.structs[0].line_number > 0);

        let _ = fs::remove_file(path);
    }

    #[test]
    fn file_analysis_cache_is_scoped_per_file_path() {
        let path_a = write_temp_rust_file(
            r#"
#[state]
enum StateA { A }
"#,
        );
        let path_b = write_temp_rust_file(
            r#"
#[state]
enum StateB { B }
"#,
        );

        let a_first = get_file_analysis(path_a.to_str().expect("a path")).expect("analysis a1");
        let a_second = get_file_analysis(path_a.to_str().expect("a path")).expect("analysis a2");
        let b_first = get_file_analysis(path_b.to_str().expect("b path")).expect("analysis b1");

        assert!(Rc::ptr_eq(&a_first, &a_second));
        assert!(!Rc::ptr_eq(&a_first, &b_first));

        let _ = fs::remove_file(path_a);
        let _ = fs::remove_file(path_b);
    }
}
