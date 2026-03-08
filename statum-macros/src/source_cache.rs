use std::collections::HashMap;
use std::cell::RefCell;
use std::fs;
use std::rc::Rc;

#[derive(Clone)]
pub struct StateEnumEntry {
    pub item: syn::ItemEnum,
    pub line_number: usize,
}

#[derive(Clone)]
pub struct MachineStructEntry {
    pub item: syn::ItemStruct,
    pub line_number: usize,
}

#[derive(Clone, Default)]
pub struct FileAnalysis {
    pub state_enums: Vec<StateEnumEntry>,
    pub machine_structs: Vec<MachineStructEntry>,
}

thread_local! {
    static FILE_ANALYSIS_CACHE: RefCell<HashMap<String, Rc<FileAnalysis>>> = RefCell::new(HashMap::new());
}

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
            syn::Item::Enum(item_enum)
                if item_enum
                    .attrs
                    .iter()
                    .any(|attr| attr.path().is_ident("state")) =>
            {
                let name = item_enum.ident.to_string();
                let line_number = find_item_line(&contents, "enum", &name)?;
                analysis.state_enums.push(StateEnumEntry {
                    item: item_enum,
                    line_number,
                });
            }
            syn::Item::Struct(item_struct)
                if item_struct
                    .attrs
                    .iter()
                    .any(|attr| attr.path().is_ident("machine")) =>
            {
                let name = item_struct.ident.to_string();
                let line_number = find_item_line(&contents, "struct", &name)?;
                analysis.machine_structs.push(MachineStructEntry {
                    item: item_struct,
                    line_number,
                });
            }
            _ => {}
        }
    }

    Some(analysis)
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
