use macro_registry::analysis::{FileAnalysis, StructEntry, get_file_analysis};
use macro_registry::callsite::current_source_info;
use macro_registry::registry::{
    RegistryDomain, StaticRegistry, ensure_loaded,
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
}

pub(super) fn get_machine_map() -> &'static RwLock<HashMap<MachinePath, MachineInfo>> {
    MACHINE_MAP.map()
}

fn get_machine(machine_path: &MachinePath) -> Option<MachineInfo> {
    MACHINE_MAP.get_cloned(machine_path)
}

fn ensure_machine_loaded(machine_path: &MachinePath) -> Option<MachineInfo> {
    ensure_loaded::<MachineRegistryDomain>(&MACHINE_MAP, machine_path)
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

    if let Some((file_path, _)) = current_source_info()
        && let Some(analysis) = get_file_analysis(&file_path)
    {
        for entry in &analysis.structs {
            if entry.item.ident != machine_name || !entry.attrs.iter().any(|attr| attr == "machine")
            {
                continue;
            }

            if let Some(info) = MachineInfo::from_item_struct_with_module(&entry.item, machine_path) {
                MACHINE_MAP.insert(machine_path.clone(), info.clone());
                return Some(info);
            }
        }
    }

    let loaded = ensure_machine_loaded(machine_path)?;
    (loaded.name == machine_name).then_some(loaded)
}

pub fn store_machine_struct(machine_info: &MachineInfo) {
    MACHINE_MAP.insert(machine_info.module_path.clone(), machine_info.clone());
}
