#![cfg_attr(
    not(any(feature = "introspection", feature = "validators")),
    allow(dead_code)
)]

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

fn loaded_machine_candidates_matching_with_crate_root<F>(
    current_crate_root: Option<&str>,
    matches: F,
) -> Vec<MachineInfo>
where
    F: Fn(&MachineInfo) -> bool,
{
    let Ok(machines) = loaded_machines().read() else {
        return Vec::new();
    };

    machines
        .iter()
        .filter(|machine| loaded_machine_is_current(machine, current_crate_root))
        .filter(|machine| matches(machine))
        .cloned()
        .collect()
}

fn loaded_machine_candidates_matching<F>(matches: F) -> Vec<MachineInfo>
where
    F: Fn(&MachineInfo) -> bool,
{
    loaded_machine_candidates_matching_with_crate_root(current_crate_root().as_deref(), matches)
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

pub fn lookup_loaded_machine_best_effort(
    machine_path: Option<&MachinePath>,
    machine_name: &str,
    current_file_path: Option<&str>,
    current_crate_root: Option<&str>,
) -> Result<MachineInfo, LoadedMachineLookupFailure> {
    if let Some(machine_path) = machine_path {
        let exact = lookup_loaded_machine_in_module(machine_path, machine_name);
        if !matches!(exact, Err(LoadedMachineLookupFailure::NotFound)) {
            return exact;
        }
    }

    if let Some(current_file_path) = current_file_path {
        let same_file = loaded_machine_candidates_matching_with_crate_root(
            current_crate_root,
            |machine| {
                machine.file_path.as_deref() == Some(current_file_path)
                    && machine.name == machine_name
            },
        );
        if !same_file.is_empty() {
            return lookup_loaded_machine_candidates(same_file);
        }
    }

    if let Some(current_crate_root) = current_crate_root {
        let same_crate = loaded_machine_candidates_matching_with_crate_root(
            Some(current_crate_root),
            |machine| machine.name == machine_name,
        );
        if !same_crate.is_empty() {
            return lookup_loaded_machine_candidates(same_crate);
        }
    }

    Err(LoadedMachineLookupFailure::NotFound)
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

#[cfg(test)]
mod tests {
    use syn::parse_quote;

    use super::{lookup_loaded_machine_best_effort, store_machine_struct};
    use crate::machine::MachineInfo;

    fn machine_info(
        item: syn::ItemStruct,
        module_path: &str,
        file_path: Option<&str>,
        crate_root: Option<&str>,
    ) -> MachineInfo {
        let module_path = crate::ModulePath(module_path.into());
        let mut info =
            MachineInfo::from_item_struct_with_module(&item, &module_path).expect("machine");
        info.file_path = file_path.map(str::to_owned);
        info.crate_root = crate_root.map(str::to_owned);
        info.file_fingerprint = None;
        info
    }

    #[test]
    fn best_effort_machine_lookup_prefers_same_file_unique_name() {
        let local: syn::ItemStruct = parse_quote! {
            pub struct __StatumLocalMachineLookupA<State> {
                pub id: String,
            }
        };
        let sibling: syn::ItemStruct = parse_quote! {
            pub struct __StatumLocalMachineLookupA<State> {
                pub id: String,
            }
        };

        store_machine_struct(&machine_info(
            local,
            "crate::alpha",
            Some("/tmp/local_machine_lookup_a.rs"),
            Some("/tmp/local_machine_lookup_crate"),
        ));
        store_machine_struct(&machine_info(
            sibling,
            "crate::beta",
            Some("/tmp/other_machine_lookup_a.rs"),
            Some("/tmp/other_machine_lookup_crate"),
        ));

        let resolved = lookup_loaded_machine_best_effort(
            None,
            "__StatumLocalMachineLookupA",
            Some("/tmp/local_machine_lookup_a.rs"),
            Some("/tmp/local_machine_lookup_crate"),
        )
        .unwrap_or_else(|_| panic!("same-file fallback should resolve"));

        assert_eq!(resolved.module_path.as_ref(), "crate::alpha");
    }

    #[test]
    fn best_effort_machine_lookup_rejects_same_file_ambiguity() {
        let first: syn::ItemStruct = parse_quote! {
            pub struct __StatumAmbiguousMachineLookupB<State> {
                pub id: String,
            }
        };
        let second: syn::ItemStruct = parse_quote! {
            pub struct __StatumAmbiguousMachineLookupB<State> {
                pub id: String,
            }
        };

        store_machine_struct(&machine_info(
            first,
            "crate::alpha",
            Some("/tmp/ambiguous_machine_lookup.rs"),
            Some("/tmp/ambiguous_machine_lookup_crate"),
        ));
        store_machine_struct(&machine_info(
            second,
            "crate::beta",
            Some("/tmp/ambiguous_machine_lookup.rs"),
            Some("/tmp/ambiguous_machine_lookup_crate"),
        ));

        let failure = lookup_loaded_machine_best_effort(
            None,
            "__StatumAmbiguousMachineLookupB",
            Some("/tmp/ambiguous_machine_lookup.rs"),
            Some("/tmp/ambiguous_machine_lookup_crate"),
        );

        assert!(matches!(
            failure,
            Err(super::LoadedMachineLookupFailure::Ambiguous(_))
        ));
    }

    #[test]
    fn best_effort_machine_lookup_falls_back_to_same_crate_unique_name() {
        let local: syn::ItemStruct = parse_quote! {
            pub struct __StatumCrateMachineLookupC<State> {
                pub id: String,
            }
        };
        let sibling: syn::ItemStruct = parse_quote! {
            pub struct __StatumCrateMachineLookupC<State> {
                pub id: String,
            }
        };

        store_machine_struct(&machine_info(
            local,
            "crate::alpha",
            Some("/tmp/crate_machine_lookup_a.rs"),
            Some("/tmp/shared_machine_lookup_crate"),
        ));
        store_machine_struct(&machine_info(
            sibling,
            "crate::beta",
            Some("/tmp/crate_machine_lookup_b.rs"),
            Some("/tmp/different_machine_lookup_crate"),
        ));

        let resolved = lookup_loaded_machine_best_effort(
            None,
            "__StatumCrateMachineLookupC",
            None,
            Some("/tmp/shared_machine_lookup_crate"),
        )
        .unwrap_or_else(|_| panic!("same-crate fallback should resolve"));

        assert_eq!(resolved.module_path.as_ref(), "crate::alpha");
    }
}
