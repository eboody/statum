use std::sync::{OnceLock, RwLock};

use super::{EnumInfo, LoadedStateLookupFailure, StateModulePath};
use crate::{current_crate_root, source_file_fingerprint};

static LOADED_STATE_ENUMS: OnceLock<RwLock<Vec<EnumInfo>>> = OnceLock::new();

fn loaded_state_enums() -> &'static RwLock<Vec<EnumInfo>> {
    LOADED_STATE_ENUMS.get_or_init(|| RwLock::new(Vec::new()))
}

fn same_loaded_state(left: &EnumInfo, right: &EnumInfo) -> bool {
    left.name == right.name
        && left.module_path.as_ref() == right.module_path.as_ref()
        && left.file_path == right.file_path
        && left.line_number == right.line_number
}

fn upsert_loaded_state(enum_info: &EnumInfo) {
    let Ok(mut states) = loaded_state_enums().write() else {
        return;
    };

    if let Some(existing) = states
        .iter_mut()
        .find(|existing| same_loaded_state(existing, enum_info))
    {
        *existing = enum_info.clone();
    } else {
        states.push(enum_info.clone());
    }
}

fn loaded_state_candidates_matching<F>(matches: F) -> Vec<EnumInfo>
where
    F: Fn(&EnumInfo) -> bool,
{
    let current_crate_root = current_crate_root();
    let Ok(states) = loaded_state_enums().read() else {
        return Vec::new();
    };

    states
        .iter()
        .filter(|state| loaded_state_is_current(state, current_crate_root.as_deref()))
        .filter(|state| matches(state))
        .cloned()
        .collect()
}

fn loaded_state_is_current(state: &EnumInfo, current_crate_root: Option<&str>) -> bool {
    if current_crate_root.is_some() && state.crate_root.as_deref() != current_crate_root {
        return false;
    }

    match (state.file_path.as_deref(), state.file_fingerprint.as_ref()) {
        (Some(file_path), Some(fingerprint)) => {
            source_file_fingerprint(file_path).as_ref() == Some(fingerprint)
        }
        _ => true,
    }
}

fn lookup_loaded_state_candidates(
    candidates: Vec<EnumInfo>,
) -> Result<EnumInfo, LoadedStateLookupFailure> {
    match candidates.len() {
        0 => Err(LoadedStateLookupFailure::NotFound),
        1 => Ok(candidates.into_iter().next().expect("single candidate")),
        _ => Err(LoadedStateLookupFailure::Ambiguous(candidates)),
    }
}

pub fn lookup_loaded_state_enum(
    enum_path: &StateModulePath,
) -> Result<EnumInfo, LoadedStateLookupFailure> {
    lookup_loaded_state_candidates(loaded_state_candidates_matching(|state| {
        state.module_path.as_ref() == enum_path.as_ref()
    }))
}

pub fn lookup_loaded_state_enum_by_name(
    enum_path: &StateModulePath,
    enum_name: &str,
) -> Result<EnumInfo, LoadedStateLookupFailure> {
    lookup_loaded_state_candidates(loaded_state_candidates_matching(|state| {
        state.module_path.as_ref() == enum_path.as_ref() && state.name == enum_name
    }))
}

pub fn format_loaded_state_candidates(candidates: &[EnumInfo]) -> String {
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

pub fn store_state_enum(enum_info: &EnumInfo) {
    upsert_loaded_state(enum_info);
}
