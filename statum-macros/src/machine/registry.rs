use std::sync::{OnceLock, RwLock};

use super::{MachineInfo, MachinePath};
use crate::{current_crate_root, source_file_fingerprint};

static LOADED_MACHINES: OnceLock<RwLock<Vec<MachineInfo>>> = OnceLock::new();

#[derive(Clone)]
pub enum LoadedMachineLookupFailure {
    NotFound,
    Ambiguous(Vec<MachineInfo>),
}

fn loaded_machines() -> &'static RwLock<Vec<MachineInfo>> {
    LOADED_MACHINES.get_or_init(|| RwLock::new(Vec::new()))
}

fn same_loaded_machine(left: &MachineInfo, right: &MachineInfo) -> bool {
    left.name == right.name
        && left.module_path.as_ref() == right.module_path.as_ref()
        && left.file_path == right.file_path
        && left.line_number == right.line_number
}

fn upsert_loaded_machine(machine_info: &MachineInfo) {
    let Ok(mut machines) = loaded_machines().write() else {
        return;
    };

    if let Some(existing) = machines
        .iter_mut()
        .find(|existing| same_loaded_machine(existing, machine_info))
    {
        *existing = machine_info.clone();
    } else {
        machines.push(machine_info.clone());
    }
}

fn loaded_machine_candidates_matching<F>(matches: F) -> Vec<MachineInfo>
where
    F: Fn(&MachineInfo) -> bool,
{
    let current_crate_root = current_crate_root();
    let Ok(machines) = loaded_machines().read() else {
        return Vec::new();
    };

    machines
        .iter()
        .filter(|machine| loaded_machine_is_current(machine, current_crate_root.as_deref()))
        .filter(|machine| matches(machine))
        .cloned()
        .collect()
}

fn loaded_machine_is_current(machine: &MachineInfo, current_crate_root: Option<&str>) -> bool {
    if current_crate_root.is_some() && machine.crate_root.as_deref() != current_crate_root {
        return false;
    }

    match (machine.file_path.as_deref(), machine.file_fingerprint.as_ref()) {
        (Some(file_path), Some(fingerprint)) => {
            source_file_fingerprint(file_path).as_ref() == Some(fingerprint)
        }
        _ => true,
    }
}

fn lookup_loaded_machine_candidates(
    candidates: Vec<MachineInfo>,
) -> Result<MachineInfo, LoadedMachineLookupFailure> {
    match candidates.len() {
        0 => Err(LoadedMachineLookupFailure::NotFound),
        1 => Ok(candidates.into_iter().next().expect("single candidate")),
        _ => Err(LoadedMachineLookupFailure::Ambiguous(candidates)),
    }
}

pub fn lookup_loaded_machine_in_module(
    machine_path: &MachinePath,
    machine_name: &str,
) -> Result<MachineInfo, LoadedMachineLookupFailure> {
    lookup_loaded_machine_candidates(loaded_machine_candidates_matching(|machine| {
        machine.module_path.as_ref() == machine_path.as_ref() && machine.name == machine_name
    }))
}

pub fn same_named_loaded_machines_elsewhere(
    machine_path: &MachinePath,
    machine_name: &str,
) -> Vec<MachineInfo> {
    loaded_machine_candidates_matching(|machine| {
        machine.name == machine_name && machine.module_path.as_ref() != machine_path.as_ref()
    })
}

pub fn format_loaded_machine_candidates(candidates: &[MachineInfo]) -> String {
    candidates
        .iter()
        .map(|candidate| {
            let file_path = candidate.file_path.as_deref().unwrap_or("<unknown file>");
            format!(
                "`{}` in `{}` ({file_path}:{})",
                candidate.name, candidate.module_path, candidate.line_number
            )
        })
        .collect::<Vec<_>>()
        .join(", ")
}

pub fn store_machine_struct(machine_info: &MachineInfo) {
    upsert_loaded_machine(machine_info);
}
