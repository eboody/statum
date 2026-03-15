use macro_registry::analysis::{FileAnalysis, StructEntry};
use macro_registry::registry::{
    NamedRegistryDomain, RegistryDomain, StaticRegistry, ensure_loaded_by_name,
};
use std::collections::HashMap;
use std::sync::RwLock;

use super::{MachineInfo, MachinePath};

static MACHINE_MAP: StaticRegistry<MachinePath, MachineInfo> = StaticRegistry::new();

struct MachineRegistryDomain;

impl RegistryDomain for MachineRegistryDomain {
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
        MachineInfo::from_item_struct_with_module(&entry.item, module_path)
    }

    fn matches_entry(entry: &Self::Entry) -> bool {
        entry.attrs.iter().any(|attr| attr == "machine")
    }

    fn entry_hint(entry: &Self::Entry) -> Option<String> {
        Some(entry.item.ident.to_string())
    }
}

impl NamedRegistryDomain for MachineRegistryDomain {
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

    ensure_loaded_by_name::<MachineRegistryDomain>(&MACHINE_MAP, machine_path, machine_name)
}

pub fn store_machine_struct(machine_info: &MachineInfo) {
    MACHINE_MAP.insert(machine_info.module_path.clone(), machine_info.clone());
}
