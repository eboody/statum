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
                let line_number = find_item_line(&contents, "enum", &name)?;
                analysis.enums.push(EnumEntry {
                    attrs: attribute_names(&item_enum.attrs),
                    item: item_enum,
                    line_number,
                });
            }
            syn::Item::Struct(item_struct) => {
                let name = item_struct.ident.to_string();
                let line_number = find_item_line(&contents, "struct", &name)?;
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
