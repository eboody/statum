use macro_registry::analysis::{FileAnalysis, StructEntry};
use macro_registry::registry;
use std::collections::HashMap;
use std::sync::RwLock;

use super::{MachineInfo, MachinePath};

static MACHINE_MAP: registry::StaticRegistry<MachinePath, MachineInfo> = registry::StaticRegistry::new();

struct MachineRegistryDomain;

impl registry::RegistryDomain for MachineRegistryDomain {
    type Key = MachinePath;
    type Value = MachineInfo;
    type Entry = StructEntry;

    fn entries(analysis: &FileAnalysis) -> &[Self::Entry] {
        &analysis.structs
    }

    fn entry_line(entry: &Self::Entry) -> usize {
        entry.line_number
    }

    fn build_value(entry: &Self::Entry, module_path: &Self::Key) -> Option<Self::Value> {
        let mut value = MachineInfo::from_item_struct_with_module(&entry.item, module_path)?;
        value.line_number = entry.line_number;
        Some(value)
    }

    fn matches_entry(entry: &Self::Entry) -> bool {
        entry.attrs.iter().any(|attr| attr == "machine")
    }

    fn entry_hint(entry: &Self::Entry) -> Option<String> {
        Some(entry.item.ident.to_string())
    }
}

impl registry::NamedRegistryDomain for MachineRegistryDomain {
    fn entry_name(entry: &Self::Entry) -> String {
        entry.item.ident.to_string()
    }

    fn value_name(value: &Self::Value) -> String {
        value.name.clone()
    }
}

pub(super) fn get_machine_map() -> &'static RwLock<HashMap<MachinePath, MachineInfo>> {
    MACHINE_MAP.map()
}

fn get_machine(machine_path: &MachinePath) -> Option<MachineInfo> {
    MACHINE_MAP.get_cloned(machine_path)
}

pub fn ensure_machine_loaded_by_name(
    machine_path: &MachinePath,
    machine_name: &str,
) -> Option<MachineInfo> {
    if let Some(existing) = get_machine(machine_path)
        && existing.name == machine_name
    {
        return Some(existing);
    }

    registry::ensure_loaded_by_name::<MachineRegistryDomain>(&MACHINE_MAP, machine_path, machine_name)
}

pub fn unique_loaded_machine_elsewhere(machine_name: &str) -> Option<MachineInfo> {
    let source = registry::SourceContext::current()?;
    let map = MACHINE_MAP.map().read().ok()?;

    let mut matches = map
        .values()
        .filter(|machine| {
            machine.name == machine_name
                && machine.file_path.as_deref() != Some(source.file_path.as_str())
        })
        .cloned()
        .collect::<Vec<_>>();

    matches.sort_by(|left, right| {
        left.name
            .cmp(&right.name)
            .then(left.module_path.as_ref().cmp(right.module_path.as_ref()))
            .then(left.file_path.cmp(&right.file_path))
            .then(left.line_number.cmp(&right.line_number))
    });
    matches.dedup_by(|left, right| {
        left.name == right.name
            && left.module_path.as_ref() == right.module_path.as_ref()
            && left.file_path == right.file_path
            && left.line_number == right.line_number
    });

    if matches.len() == 1 {
        matches.pop()
    } else {
        None
    }
}

pub fn store_machine_struct(machine_info: &MachineInfo) {
    MACHINE_MAP.insert(machine_info.module_path.clone(), machine_info.clone());
}
