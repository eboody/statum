use std::env;
use std::ffi::OsString;
use std::fmt;
use std::fmt::Write as _;
use std::fs;
use std::io::{self, Write as _};
use std::path::Component;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitStatus, Stdio};
use std::thread;

use cargo_metadata::{Metadata, MetadataCommand, Package, PackageId};
use statum_graph::codebase::render::DiagramError as CodebaseDiagramError;
use statum_graph::{CodebaseDoc, CodebaseMachine, CodebaseRelation};

mod heuristics;
mod inspect;
mod suggestions;

pub use heuristics::{
    collect_heuristic_overlay, HeuristicDiagnostic, HeuristicEvidenceKind,
    HeuristicMachineRelationGroup, HeuristicOverlay, HeuristicRelation, HeuristicRelationCount,
    HeuristicRelationDetail, HeuristicRelationSource, HeuristicStatusKind, InspectPackageSource,
};
pub use suggestions::{
    collect_composition_suggestions, render_composition_suggestions, CompositionSuggestion,
    CompositionSuggestionKind, CompositionSuggestionOverlay, CompositionSuggestionSeverity,
};

const GRAPH_EXTENSIONS: [&str; 4] = ["mmd", "dot", "puml", "json"];
const GRAPH_PACKAGE_NAME: &str = "statum-graph";
const HELPER_PACKAGE_NAME: &str = "cargo-statum-graph";
const STATUM_WORKSPACE_PACKAGES: [&str; 6] = [
    "macro_registry",
    "module_path_extractor",
    "statum",
    "statum-core",
    "statum-graph",
    "statum-macros",
];
const RUNNER_SCHEMA_VERSION: u32 = 1;
const NO_LINKED_MACHINES_MESSAGE: &str = "statum-graph: no linked state machines were found in the target workspace. This can mean the workspace has no Statum machines, or that it depends on incompatible `statum`, `statum-core`, or `statum-graph` versions so linked inventories do not unify. If you expected machines here, ensure those crates use compatible versions.";
const NO_TTY_INSPECT_MESSAGE: &str =
    "statum-graph inspect requires an interactive terminal on stdin and stdout.";

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExportOptions {
    pub input_path: PathBuf,
    pub package: Option<String>,
    pub out_dir: Option<PathBuf>,
    pub stem: String,
    pub patch_statum_root: Option<PathBuf>,
}

#[doc(hidden)]
pub type Options = ExportOptions;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InspectOptions {
    pub input_path: PathBuf,
    pub package: Option<String>,
    pub patch_statum_root: Option<PathBuf>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SuggestOptions {
    pub input_path: PathBuf,
    pub package: Option<String>,
    pub patch_statum_root: Option<PathBuf>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StateDiagramOptions {
    pub input_path: PathBuf,
    pub package: Option<String>,
    pub machine: Option<String>,
    pub patch_statum_root: Option<PathBuf>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SequenceDiagramOptions {
    pub input_path: PathBuf,
    pub package: Option<String>,
    pub relation: Option<usize>,
    pub from_machine: Option<String>,
    pub to_machine: Option<String>,
    pub patch_statum_root: Option<PathBuf>,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct RelationDiagramSelection<'a> {
    pub relation_index: Option<usize>,
    pub from_machine: Option<&'a str>,
    pub to_machine: Option<&'a str>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DiagramSelectionError {
    MachineSelectionRequired {
        available: Vec<String>,
    },
    MachineNotFound {
        selector: String,
        available: Vec<String>,
    },
    MachineAmbiguous {
        selector: String,
        matches: Vec<String>,
    },
    NoExactRelations,
    RelationSelectionRequired {
        relation_count: usize,
    },
    ConflictingRelationSelectors,
    RelationPairSelectionIncomplete,
    RelationNotFound {
        index: usize,
    },
    RelationPairNotFound {
        from: String,
        to: String,
    },
    RelationPairAmbiguous {
        from: String,
        to: String,
        matches: Vec<String>,
    },
    Render {
        source: CodebaseDiagramError,
    },
}

#[derive(Debug)]
pub enum Error {
    CurrentDir(io::Error),
    Metadata(cargo_metadata::Error),
    PackageNotFound {
        manifest_path: PathBuf,
        package: String,
    },
    AmbiguousPackage {
        manifest_path: PathBuf,
        candidates: Vec<String>,
    },
    PackageHasNoLibrary {
        manifest_path: PathBuf,
        package: String,
    },
    AmbiguousPatchStatumRoots {
        manifest_path: PathBuf,
        candidates: Vec<PathBuf>,
    },
    InvalidStem {
        stem: String,
    },
    InvalidDiagramSelection {
        message: String,
    },
    NonUtf8Path {
        role: &'static str,
        path: PathBuf,
    },
    Io {
        action: &'static str,
        path: PathBuf,
        source: io::Error,
    },
    RunnerFailed {
        operation: &'static str,
        manifest_path: PathBuf,
        status: ExitStatus,
        details: Option<String>,
        diagnostics_reported: bool,
    },
}

impl fmt::Display for Error {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::CurrentDir(source) => {
                write!(formatter, "failed to read current directory: {source}")
            }
            Self::Metadata(source) => write!(formatter, "failed to load cargo metadata: {source}"),
            Self::PackageNotFound {
                manifest_path,
                package,
            } => write!(
                formatter,
                "manifest `{}` does not contain package `{package}`",
                manifest_path.display()
            ),
            Self::AmbiguousPackage {
                manifest_path,
                candidates,
            } => {
                if candidates.is_empty() {
                    write!(
                        formatter,
                        "manifest `{}` does not contain a library package",
                        manifest_path.display()
                    )
                } else {
                    write!(
                        formatter,
                        "manifest `{}` does not identify one library package; choose one of: {}",
                        manifest_path.display(),
                        candidates.join(", ")
                    )
                }
            }
            Self::PackageHasNoLibrary {
                manifest_path,
                package,
            } => write!(
                formatter,
                "package `{package}` from manifest `{}` does not expose a library target",
                manifest_path.display()
            ),
            Self::AmbiguousPatchStatumRoots {
                manifest_path,
                candidates,
            } => write!(
                formatter,
                "manifest `{}` reaches multiple local Statum workspace roots; use --patch-statum-root to choose one: {}",
                manifest_path.display(),
                candidates
                    .iter()
                    .map(|candidate| candidate.display().to_string())
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
            Self::InvalidStem { stem } => write!(
                formatter,
                "invalid output stem `{stem}`: expected a simple file name without path separators"
            ),
            Self::InvalidDiagramSelection { message } => formatter.write_str(message),
            Self::NonUtf8Path { role, path } => write!(
                formatter,
                "cannot generate runner {role} from non-UTF-8 path `{}`",
                path.display()
            ),
            Self::Io {
                action,
                path,
                source,
            } => write!(
                formatter,
                "failed to {action} `{}`: {source}",
                path.display()
            ),
            Self::RunnerFailed {
                operation,
                manifest_path,
                status,
                details,
                diagnostics_reported: _,
            } => match details {
                Some(details) => write!(
                    formatter,
                    "{operation} for `{}` failed:\n{details}",
                    manifest_path.display()
                ),
                None => write!(
                    formatter,
                    "{operation} for `{}` failed with status {status}",
                    manifest_path.display()
                ),
            },
        }
    }
}

impl Error {
    pub fn diagnostics_reported(&self) -> bool {
        matches!(
            self,
            Self::RunnerFailed {
                diagnostics_reported: true,
                ..
            }
        )
    }

    pub fn post_diagnostics_note(&self) -> Option<String> {
        match self {
            Self::RunnerFailed {
                operation,
                manifest_path,
                details,
                diagnostics_reported: true,
                ..
            } if details
                .as_deref()
                .is_some_and(runner_failure_details_look_like_build_failure) =>
            {
                Some(format!(
                    "{operation} stopped while building the generated Statum runner against `{}`.\nThe target workspace must compile before cargo-statum-graph can continue.\nFix the compiler error above and rerun, or narrow the run with `--package`.",
                    manifest_path.display()
                ))
            }
            _ => None,
        }
    }
}

impl std::error::Error for Error {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::CurrentDir(source) => Some(source),
            Self::Metadata(source) => Some(source),
            Self::Io { source, .. } => Some(source),
            Self::PackageNotFound { .. }
            | Self::AmbiguousPackage { .. }
            | Self::PackageHasNoLibrary { .. }
            | Self::AmbiguousPatchStatumRoots { .. }
            | Self::InvalidStem { .. }
            | Self::InvalidDiagramSelection { .. }
            | Self::NonUtf8Path { .. }
            | Self::RunnerFailed { .. } => None,
        }
    }
}

impl fmt::Display for DiagramSelectionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MachineSelectionRequired { available } => write!(
                formatter,
                "machine selector required; available linked machines: {}",
                available.join(", ")
            ),
            Self::MachineNotFound {
                selector,
                available,
            } => write!(
                formatter,
                "machine selector `{selector}` did not match any linked machine; available machines: {}",
                available.join(", ")
            ),
            Self::MachineAmbiguous { selector, matches } => write!(
                formatter,
                "machine selector `{selector}` matched multiple linked machines: {}",
                matches.join(", ")
            ),
            Self::NoExactRelations => {
                formatter.write_str("no exact linked relations are available for sequence export")
            }
            Self::RelationSelectionRequired { relation_count } => write!(
                formatter,
                "relation selector required; linked codebase has {relation_count} exact relations, so choose --relation INDEX or --from/--to"
            ),
            Self::ConflictingRelationSelectors => {
                formatter.write_str("choose either --relation or --from/--to, not both")
            }
            Self::RelationPairSelectionIncomplete => {
                formatter.write_str("relation pair selection requires both --from and --to")
            }
            Self::RelationNotFound { index } => {
                write!(formatter, "exact relation index {index} is missing")
            }
            Self::RelationPairNotFound { from, to } => {
                write!(formatter, "no exact relation matched `{from}` -> `{to}`")
            }
            Self::RelationPairAmbiguous { from, to, matches } => write!(
                formatter,
                "machine pair `{from}` -> `{to}` matched multiple exact relations: {}",
                matches.join(", ")
            ),
            Self::Render { source } => source.fmt(formatter),
        }
    }
}

impl std::error::Error for DiagramSelectionError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Render { source } => Some(source),
            Self::MachineSelectionRequired { .. }
            | Self::MachineNotFound { .. }
            | Self::MachineAmbiguous { .. }
            | Self::NoExactRelations
            | Self::RelationSelectionRequired { .. }
            | Self::ConflictingRelationSelectors
            | Self::RelationPairSelectionIncomplete
            | Self::RelationNotFound { .. }
            | Self::RelationPairNotFound { .. }
            | Self::RelationPairAmbiguous { .. } => None,
        }
    }
}

pub fn export(options: ExportOptions) -> Result<Vec<PathBuf>, Error> {
    validate_output_stem(&options.stem)?;
    let prepared = prepare_run(
        &options.input_path,
        options.package.as_deref(),
        options.patch_statum_root.as_deref(),
    )?;
    let out_dir = resolve_out_dir(&prepared.input, options.out_dir.as_deref())?;
    let runner = materialize_cached_runner(
        &prepared.target_directory,
        &prepared.selections,
        prepared.patch_root.as_deref(),
    )?;
    run_runner(
        &runner.runner,
        &prepared.input.manifest_path,
        "workspace export",
        RunnerStdio::Captured,
        &[
            OsString::from("export"),
            out_dir.as_os_str().to_owned(),
            OsString::from(options.stem.clone()),
        ],
    )?;

    Ok(bundle_paths(&out_dir, &options.stem))
}

#[doc(hidden)]
pub fn run(options: Options) -> Result<Vec<PathBuf>, Error> {
    export(options)
}

pub fn inspect(options: InspectOptions) -> Result<(), Error> {
    let prepared = prepare_run(
        &options.input_path,
        options.package.as_deref(),
        options.patch_statum_root.as_deref(),
    )?;
    let workspace_label = prepared.input.manifest_path.display().to_string();
    let runner = materialize_cached_runner(
        &prepared.target_directory,
        &prepared.selections,
        prepared.patch_root.as_deref(),
    )?;
    run_runner(
        &runner.runner,
        &prepared.input.manifest_path,
        "inspect session",
        RunnerStdio::Inherited,
        &[OsString::from("inspect"), OsString::from(workspace_label)],
    )
}

pub fn suggest(options: SuggestOptions) -> Result<String, Error> {
    let prepared = prepare_run(
        &options.input_path,
        options.package.as_deref(),
        options.patch_statum_root.as_deref(),
    )?;
    let runner = materialize_cached_runner(
        &prepared.target_directory,
        &prepared.selections,
        prepared.patch_root.as_deref(),
    )?;

    run_runner_captured(
        runner.runner.manifest_path,
        &prepared.target_directory,
        &prepared.input.manifest_path,
        "composition suggestion report",
        &[OsString::from("suggest")],
    )
}

pub fn state_diagram(options: StateDiagramOptions) -> Result<String, Error> {
    let prepared = prepare_run(
        &options.input_path,
        options.package.as_deref(),
        options.patch_statum_root.as_deref(),
    )?;
    let runner = materialize_cached_runner(
        &prepared.target_directory,
        &prepared.selections,
        prepared.patch_root.as_deref(),
    )?;
    let mut runtime_args = vec![OsString::from("state-diagram")];
    match options.machine {
        Some(machine) => {
            runtime_args.push(OsString::from("machine"));
            runtime_args.push(OsString::from(machine));
        }
        None => runtime_args.push(OsString::from("auto")),
    }

    run_runner_captured(
        runner.runner.manifest_path,
        &prepared.target_directory,
        &prepared.input.manifest_path,
        "machine state diagram",
        &runtime_args,
    )
}

pub fn sequence_diagram(options: SequenceDiagramOptions) -> Result<String, Error> {
    let has_pair_selector = options.from_machine.is_some() || options.to_machine.is_some();
    if options.relation.is_some() && has_pair_selector {
        return Err(Error::InvalidDiagramSelection {
            message: "choose either --relation or --from/--to, not both".to_owned(),
        });
    }
    if options.from_machine.is_some() ^ options.to_machine.is_some() {
        return Err(Error::InvalidDiagramSelection {
            message: "relation pair selection requires both --from and --to".to_owned(),
        });
    }

    let prepared = prepare_run(
        &options.input_path,
        options.package.as_deref(),
        options.patch_statum_root.as_deref(),
    )?;
    let runner = materialize_cached_runner(
        &prepared.target_directory,
        &prepared.selections,
        prepared.patch_root.as_deref(),
    )?;
    let mut runtime_args = vec![OsString::from("sequence-diagram")];
    if let Some(relation) = options.relation {
        runtime_args.push(OsString::from("index"));
        runtime_args.push(OsString::from(relation.to_string()));
    } else if let (Some(from_machine), Some(to_machine)) =
        (options.from_machine, options.to_machine)
    {
        runtime_args.push(OsString::from("pair"));
        runtime_args.push(OsString::from(from_machine));
        runtime_args.push(OsString::from(to_machine));
    } else {
        runtime_args.push(OsString::from("auto"));
    }

    run_runner_captured(
        runner.runner.manifest_path,
        &prepared.target_directory,
        &prepared.input.manifest_path,
        "relation sequence diagram",
        &runtime_args,
    )
}

pub fn render_machine_state_diagram(
    doc: &CodebaseDoc,
    machine_selector: Option<&str>,
) -> Result<String, DiagramSelectionError> {
    let machine = resolve_machine(doc, machine_selector)?;
    statum_graph::codebase::render::mermaid_machine_state(doc, machine.index)
        .map_err(|source| DiagramSelectionError::Render { source })
}

pub fn render_relation_sequence_diagram(
    doc: &CodebaseDoc,
    selection: RelationDiagramSelection<'_>,
) -> Result<String, DiagramSelectionError> {
    let relation_index = resolve_relation_index(doc, selection)?;
    statum_graph::codebase::render::mermaid_relation_sequence(doc, relation_index)
        .map_err(|source| DiagramSelectionError::Render { source })
}

pub fn run_inspector(
    doc: CodebaseDoc,
    heuristic: HeuristicOverlay,
    workspace_label: String,
) -> Result<(), InspectError> {
    let suggestions = suggestions::collect_composition_suggestions(&doc, &heuristic);
    inspect::run(doc, heuristic, suggestions, workspace_label).map_err(InspectError::Io)
}

fn resolve_machine<'a>(
    doc: &'a CodebaseDoc,
    selector: Option<&str>,
) -> Result<&'a CodebaseMachine, DiagramSelectionError> {
    match selector {
        Some(selector) => {
            if let Some(machine) = doc
                .machines()
                .iter()
                .find(|machine| machine.rust_type_path == selector)
            {
                return Ok(machine);
            }

            let matches = doc
                .machines()
                .iter()
                .filter(|machine| machine.rust_type_path.ends_with(selector))
                .collect::<Vec<_>>();
            match matches.as_slice() {
                [machine] => Ok(*machine),
                [] => Err(DiagramSelectionError::MachineNotFound {
                    selector: selector.to_owned(),
                    available: available_machines(doc),
                }),
                _ => Err(DiagramSelectionError::MachineAmbiguous {
                    selector: selector.to_owned(),
                    matches: matches
                        .iter()
                        .map(|machine| machine.rust_type_path.to_owned())
                        .collect(),
                }),
            }
        }
        None => match doc.machines() {
            [machine] => Ok(machine),
            _ => Err(DiagramSelectionError::MachineSelectionRequired {
                available: available_machines(doc),
            }),
        },
    }
}

fn resolve_relation_index(
    doc: &CodebaseDoc,
    selection: RelationDiagramSelection<'_>,
) -> Result<usize, DiagramSelectionError> {
    match (
        selection.relation_index,
        selection.from_machine,
        selection.to_machine,
    ) {
        (Some(_), Some(_), _) | (Some(_), _, Some(_)) => {
            Err(DiagramSelectionError::ConflictingRelationSelectors)
        }
        (Some(index), None, None) => doc
            .relation(index)
            .map(|relation| relation.index)
            .ok_or(DiagramSelectionError::RelationNotFound { index }),
        (None, Some(_), None) | (None, None, Some(_)) => {
            Err(DiagramSelectionError::RelationPairSelectionIncomplete)
        }
        (None, Some(from_selector), Some(to_selector)) => {
            let from_machine = resolve_machine(doc, Some(from_selector))?;
            let to_machine = resolve_machine(doc, Some(to_selector))?;
            let matches = doc
                .relations()
                .iter()
                .filter(|relation| {
                    relation.source_machine() == from_machine.index
                        && relation.target_machine == to_machine.index
                })
                .collect::<Vec<_>>();
            match matches.as_slice() {
                [] => Err(DiagramSelectionError::RelationPairNotFound {
                    from: from_machine.rust_type_path.to_owned(),
                    to: to_machine.rust_type_path.to_owned(),
                }),
                [relation] => Ok(relation.index),
                _ => Err(DiagramSelectionError::RelationPairAmbiguous {
                    from: from_machine.rust_type_path.to_owned(),
                    to: to_machine.rust_type_path.to_owned(),
                    matches: matches
                        .iter()
                        .map(|relation| relation_selection_label(doc, relation))
                        .collect(),
                }),
            }
        }
        (None, None, None) => match doc.relations() {
            [] => Err(DiagramSelectionError::NoExactRelations),
            [relation] => Ok(relation.index),
            relations => Err(DiagramSelectionError::RelationSelectionRequired {
                relation_count: relations.len(),
            }),
        },
    }
}

fn available_machines(doc: &CodebaseDoc) -> Vec<String> {
    doc.machines()
        .iter()
        .map(|machine| machine.rust_type_path.to_owned())
        .collect()
}

fn relation_selection_label(doc: &CodebaseDoc, relation: &CodebaseRelation) -> String {
    let Some(detail) = doc.relation_detail(relation.index) else {
        return format!("#{} <missing relation detail>", relation.index);
    };
    let source_machine = detail.source_machine.rust_type_path;
    let target_machine = detail.target_machine.rust_type_path;
    let source = match relation.source {
        statum_graph::CodebaseRelationSource::StatePayload { field_name, .. } => {
            match (detail.source_state.map(|state| state.rust_name), field_name) {
                (Some(state_name), Some(field_name)) => {
                    format!("state payload {state_name}::{field_name}")
                }
                (Some(state_name), None) => format!("state payload {state_name}"),
                (None, Some(field_name)) => format!("state payload <missing>::{field_name}"),
                (None, None) => "state payload <missing>".to_owned(),
            }
        }
        statum_graph::CodebaseRelationSource::MachineField {
            field_name,
            field_index,
            ..
        } => match field_name {
            Some(field_name) => format!("machine field {field_name}"),
            None => format!("machine field #{field_index}"),
        },
        statum_graph::CodebaseRelationSource::TransitionParam {
            param_index,
            param_name,
            ..
        } => {
            let transition_name = detail
                .source_transition
                .map(|transition| transition.method_name)
                .unwrap_or("<missing transition>");
            match detail.source_state.map(|state| state.rust_name) {
                Some(state_name) => match param_name {
                    Some(param_name) => {
                        format!("transition param {state_name}::{transition_name}({param_name})")
                    }
                    None => {
                        format!("transition param {state_name}::{transition_name}[#{param_index}]")
                    }
                },
                None => match param_name {
                    Some(param_name) => {
                        format!("transition param {transition_name}({param_name})")
                    }
                    None => format!("transition param {transition_name}[#{param_index}]"),
                },
            }
        }
    };
    let via = relation
        .attested_via
        .as_ref()
        .map(|attested_via| {
            format!(
                " via {}::{}",
                attested_via.via_module_path, attested_via.route_name
            )
        })
        .unwrap_or_default();

    format!(
        "#{} {} -> {} [{} / {}] {}{}",
        relation.index,
        source_machine,
        target_machine,
        relation.semantic.display_label(),
        relation.basis.display_label(),
        source,
        via
    )
}

#[derive(Debug)]
pub enum InspectError {
    Io(io::Error),
}

impl fmt::Display for InspectError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(source) => write!(formatter, "{source}"),
        }
    }
}

impl std::error::Error for InspectError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io(source) => Some(source),
        }
    }
}

fn load_metadata(manifest_path: &Path) -> Result<Metadata, Error> {
    MetadataCommand::new()
        .manifest_path(manifest_path)
        .no_deps()
        .exec()
        .map_err(Error::Metadata)
}

fn load_metadata_with_deps(manifest_path: &Path) -> Result<Metadata, Error> {
    MetadataCommand::new()
        .manifest_path(manifest_path)
        .exec()
        .map_err(Error::Metadata)
}

fn select_packages(
    metadata: &Metadata,
    input: &ResolvedInput,
    requested: Option<&str>,
) -> Result<Vec<SelectedPackage>, Error> {
    let manifest_path = input.manifest_path.as_path();
    if let Some(package) = requested {
        let selected = metadata
            .packages
            .iter()
            .find(|candidate| candidate.name.as_ref() == package)
            .ok_or_else(|| Error::PackageNotFound {
                manifest_path: manifest_path.to_path_buf(),
                package: package.to_owned(),
            })?;
        return SelectedPackage::new(selected, manifest_path).map(|package| vec![package]);
    }

    if manifest_path == workspace_root_manifest(metadata) {
        let mut packages = workspace_packages(metadata, &metadata.workspace_members)
            .into_iter()
            .filter(|package| has_library_target(package))
            .collect::<Vec<_>>();
        packages.sort_by(|left, right| {
            left.name
                .as_ref()
                .cmp(right.name.as_ref())
                .then_with(|| left.manifest_path.cmp(&right.manifest_path))
        });

        if packages.is_empty() {
            return Err(Error::AmbiguousPackage {
                manifest_path: manifest_path.to_path_buf(),
                candidates: Vec::new(),
            });
        }

        return packages
            .into_iter()
            .map(|package| SelectedPackage::new(package, manifest_path))
            .collect();
    }

    if let Some(root_package) = metadata.root_package() {
        if has_library_target(root_package) {
            return SelectedPackage::new(root_package, manifest_path).map(|package| vec![package]);
        }
    }

    let default_members = workspace_packages(metadata, &metadata.workspace_default_members);
    let default_library_members = default_members
        .into_iter()
        .filter(|package| has_library_target(package))
        .collect::<Vec<_>>();
    if default_library_members.len() == 1 {
        return SelectedPackage::new(default_library_members[0], manifest_path)
            .map(|package| vec![package]);
    }

    let workspace_members = workspace_packages(metadata, &metadata.workspace_members);
    let library_members = workspace_members
        .into_iter()
        .filter(|package| has_library_target(package))
        .collect::<Vec<_>>();

    match library_members.as_slice() {
        [package] => SelectedPackage::new(package, manifest_path).map(|package| vec![package]),
        [] => Err(Error::AmbiguousPackage {
            manifest_path: manifest_path.to_path_buf(),
            candidates: Vec::new(),
        }),
        _ => Err(Error::AmbiguousPackage {
            manifest_path: manifest_path.to_path_buf(),
            candidates: library_members
                .iter()
                .map(|package| package.name.to_string())
                .collect(),
        }),
    }
}

fn workspace_root_manifest(metadata: &Metadata) -> PathBuf {
    normalize_absolute_path(&metadata.workspace_root.as_std_path().join("Cargo.toml"))
}

fn workspace_packages<'a>(metadata: &'a Metadata, ids: &[PackageId]) -> Vec<&'a Package> {
    ids.iter()
        .filter_map(|id| metadata.packages.iter().find(|package| package.id == *id))
        .collect()
}

fn has_library_target(package: &Package) -> bool {
    library_target(package).is_some()
}

fn library_target(package: &Package) -> Option<&cargo_metadata::Target> {
    package.targets.iter().find(|target| {
        target.kind.iter().any(|kind| {
            matches!(
                kind,
                cargo_metadata::TargetKind::Lib
                    | cargo_metadata::TargetKind::RLib
                    | cargo_metadata::TargetKind::DyLib
            )
        })
    })
}

fn resolve_out_dir(input: &ResolvedInput, out_dir: Option<&Path>) -> Result<PathBuf, Error> {
    match out_dir {
        Some(path) => absolutize(path).map_err(Error::CurrentDir),
        None => Ok(input.default_output_dir.clone()),
    }
}

fn prepare_run(
    input_path: &Path,
    requested_package: Option<&str>,
    patch_statum_root: Option<&Path>,
) -> Result<PreparedRun, Error> {
    let input_path = absolutize(input_path).map_err(Error::CurrentDir)?;
    let input = resolve_input(&input_path);
    let metadata = load_metadata(&input.manifest_path)?;
    let selections = select_packages(&metadata, &input, requested_package)?;
    let target_directory = normalize_absolute_path(metadata.target_directory.as_std_path());
    let patch_root = match patch_statum_root {
        Some(path) => Some(absolutize(path).map_err(Error::CurrentDir)?),
        None => detect_patch_root().or(detect_patch_root_from_target_workspace(
            &input.manifest_path,
        )?),
    };

    Ok(PreparedRun {
        input,
        selections,
        target_directory,
        patch_root,
    })
}

fn detect_patch_root() -> Option<PathBuf> {
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let candidate = manifest_dir.parent()?;
    if looks_like_statum_workspace(candidate) {
        Some(candidate.to_path_buf())
    } else {
        None
    }
}

fn looks_like_statum_workspace(path: &Path) -> bool {
    [
        "Cargo.toml",
        "statum/Cargo.toml",
        "statum-core/Cargo.toml",
        "statum-graph/Cargo.toml",
        "statum-macros/Cargo.toml",
    ]
    .into_iter()
    .all(|relative| path.join(relative).is_file())
}

fn detect_patch_root_from_target_workspace(manifest_path: &Path) -> Result<Option<PathBuf>, Error> {
    let metadata = load_metadata_with_deps(manifest_path)?;
    let manifest_dirs = metadata
        .packages
        .iter()
        .filter(|package| is_statum_workspace_package(package.name.as_ref()))
        .filter_map(|package| {
            package
                .manifest_path
                .as_std_path()
                .parent()
                .map(normalize_absolute_path)
        })
        .collect::<Vec<_>>();
    detect_patch_root_from_manifest_dirs(manifest_path, manifest_dirs)
}

fn is_statum_workspace_package(package_name: &str) -> bool {
    STATUM_WORKSPACE_PACKAGES.contains(&package_name)
}

fn detect_patch_root_from_manifest_dirs(
    manifest_path: &Path,
    manifest_dirs: impl IntoIterator<Item = PathBuf>,
) -> Result<Option<PathBuf>, Error> {
    let mut candidates = manifest_dirs
        .into_iter()
        .filter_map(|manifest_dir| {
            manifest_dir
                .parent()
                .map(normalize_absolute_path)
                .and_then(|root| {
                    if looks_like_statum_workspace(&root) {
                        Some(root)
                    } else {
                        None
                    }
                })
        })
        .collect::<Vec<_>>();
    candidates.sort();
    candidates.dedup();

    match candidates.as_slice() {
        [] => Ok(None),
        [root] => Ok(Some(root.clone())),
        _ => Err(Error::AmbiguousPatchStatumRoots {
            manifest_path: manifest_path.to_path_buf(),
            candidates,
        }),
    }
}

fn resolve_input(path: &Path) -> ResolvedInput {
    if path.is_dir() {
        ResolvedInput {
            manifest_path: path.join("Cargo.toml"),
            default_output_dir: path.to_path_buf(),
        }
    } else {
        ResolvedInput {
            manifest_path: path.to_path_buf(),
            default_output_dir: path
                .parent()
                .expect("absolute file path should have a parent")
                .to_path_buf(),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct CachedRunner {
    key: String,
    home_dir: PathBuf,
    manifest_path: PathBuf,
    target_directory: PathBuf,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct MaterializedCachedRunner {
    runner: CachedRunner,
    manifest_rewritten: bool,
    source_rewritten: bool,
}

fn materialize_cached_runner(
    target_directory: &Path,
    selections: &[SelectedPackage],
    patch_root: Option<&Path>,
) -> Result<MaterializedCachedRunner, Error> {
    let key = runner_key(selections, patch_root)?;
    let home_dir = cached_runner_home(target_directory, &key);
    let src_dir = home_dir.join("src");
    fs::create_dir_all(&src_dir).map_err(|source| Error::Io {
        action: "create cached runner source directory",
        path: src_dir.clone(),
        source,
    })?;

    let manifest_path = home_dir.join("Cargo.toml");
    let manifest_rewritten = write_file_if_changed(
        &manifest_path,
        &build_runner_manifest(selections, patch_root)?,
        "write cached runner manifest",
    )?;

    let main_path = src_dir.join("main.rs");
    let source_rewritten = write_file_if_changed(
        &main_path,
        &build_runner_main(selections)?,
        "write cached runner source",
    )?;

    Ok(MaterializedCachedRunner {
        runner: CachedRunner {
            key,
            home_dir,
            manifest_path,
            target_directory: target_directory.to_path_buf(),
        },
        manifest_rewritten,
        source_rewritten,
    })
}

fn build_runner_manifest(
    selections: &[SelectedPackage],
    patch_root: Option<&Path>,
) -> Result<String, Error> {
    let selections = normalized_runner_selections(selections);
    let mut manifest = String::from(
        "[package]\nname = \"statum-graph-runner\"\nversion = \"0.0.0\"\nedition = \"2021\"\npublish = false\n\n[workspace]\n\n[dependencies]\n",
    );
    for (index, selection) in selections.iter().enumerate() {
        manifest.push_str(&format!(
            "{} = {{ package = {}, path = {} }}\n",
            selection.dependency_alias(index),
            toml_str(&selection.package_name),
            toml_path(&selection.manifest_dir, "dependency package path")?,
        ));
    }

    match patch_root {
        Some(root) => {
            if !selections
                .iter()
                .any(|selection| selection.package_name == GRAPH_PACKAGE_NAME)
            {
                manifest.push_str(&format!(
                    "statum-graph = {{ path = {} }}\n",
                    toml_path(root.join(GRAPH_PACKAGE_NAME), "patched statum-graph path")?
                ));
            }
            if !selections
                .iter()
                .any(|selection| selection.package_name == HELPER_PACKAGE_NAME)
            {
                manifest.push_str(&format!(
                    "cargo-statum-graph = {{ path = {} }}\n",
                    toml_path(
                        root.join(HELPER_PACKAGE_NAME),
                        "patched cargo-statum-graph path",
                    )?
                ));
            }
            push_patch_tables(&mut manifest, root)?;
        }
        None => {
            if !selections
                .iter()
                .any(|selection| selection.package_name == GRAPH_PACKAGE_NAME)
            {
                manifest.push_str(&format!(
                    "statum-graph = {{ version = {} }}\n",
                    toml_str(&format!("={}", env!("CARGO_PKG_VERSION")))
                ));
            }
            if !selections
                .iter()
                .any(|selection| selection.package_name == HELPER_PACKAGE_NAME)
            {
                manifest.push_str(&format!(
                    "cargo-statum-graph = {{ version = {} }}\n",
                    toml_str(&format!("={}", env!("CARGO_PKG_VERSION")))
                ));
            }
        }
    }

    Ok(manifest)
}

fn push_patch_tables(manifest: &mut String, root: &Path) -> Result<(), Error> {
    for source in ["crates-io", "https://github.com/eboody/statum"] {
        if source == "crates-io" {
            manifest.push_str("\n[patch.crates-io]\n");
        } else {
            manifest.push_str(&format!("\n[patch.{}]\n", toml_str(source)));
        }
        for package in [
            "macro_registry",
            "module_path_extractor",
            "statum",
            "statum-core",
            "statum-graph",
            "statum-macros",
        ] {
            manifest.push_str(&format!(
                "{package} = {{ path = {} }}\n",
                toml_path(root.join(package), "patched workspace package path")?
            ));
        }
    }

    Ok(())
}

fn build_runner_main(selections: &[SelectedPackage]) -> Result<String, Error> {
    let selections = normalized_runner_selections(selections);
    let mut source = String::from("#[allow(unused_imports)]\n");
    source.push_str("use std::ffi::OsString;\n");
    source.push_str("use std::io::IsTerminal as _;\n");
    source.push_str("use std::path::PathBuf;\n");
    for (index, selection) in selections.iter().enumerate() {
        source.push_str(&format!(
            "use {} as _;\n",
            selection.dependency_alias(index)
        ));
    }
    source.push_str("\nfn main() -> std::process::ExitCode {\n");
    source.push_str("    match run() {\n");
    source.push_str("        Ok(()) => std::process::ExitCode::SUCCESS,\n");
    source.push_str("        Err(error) => {\n");
    source.push_str("            eprintln!(\"{}\", error);\n");
    source.push_str("            std::process::ExitCode::FAILURE\n");
    source.push_str("        }\n");
    source.push_str("    }\n");
    source.push_str("}\n\n");
    source.push_str("fn run() -> Result<(), Box<dyn std::error::Error>> {\n");
    source.push_str("    let mut args = std::env::args_os();\n");
    source.push_str("    let _binary = args.next();\n");
    source.push_str("    let command = take_string_arg(&mut args, \"runner command\")?;\n");
    source.push_str("    let doc = statum_graph::CodebaseDoc::linked()?;\n");
    source.push_str("    if doc.machines().is_empty() {\n");
    source.push_str("        return Err(std::io::Error::other(");
    source.push_str(&rust_str(NO_LINKED_MACHINES_MESSAGE));
    source.push_str(").into());\n");
    source.push_str("    }\n");
    source.push_str("    match command.as_str() {\n");
    source.push_str("        \"inspect\" => {\n");
    source.push_str(
        "            let workspace_label = take_string_arg(&mut args, \"workspace label\")?;\n",
    );
    source.push_str("            ensure_no_extra_args(&mut args, \"inspect\")?;\n");
    source.push_str(
        "            if !std::io::stdin().is_terminal() || !std::io::stdout().is_terminal() {\n",
    );
    source.push_str("                return Err(std::io::Error::other(");
    source.push_str(&rust_str(NO_TTY_INSPECT_MESSAGE));
    source.push_str(").into());\n");
    source.push_str("            }\n");
    source.push_str("            let heuristic = cargo_statum_graph::collect_heuristic_overlay(\n");
    source.push_str("                &doc,\n");
    source.push_str("                &[\n");
    source.push_str(&inspect_package_sources_literal(&selections)?);
    source.push_str("                ],\n");
    source.push_str("            );\n");
    source.push_str(
        "            cargo_statum_graph::run_inspector(doc, heuristic, workspace_label)?;\n",
    );
    source.push_str("        }\n");
    source.push_str("        \"export\" | \"codebase\" => {\n");
    source.push_str(
        "            let out_dir = PathBuf::from(take_os_arg(&mut args, \"output directory\")?);\n",
    );
    source.push_str("            let stem = take_string_arg(&mut args, \"output stem\")?;\n");
    source.push_str("            ensure_no_extra_args(&mut args, \"export\")?;\n");
    source.push_str(
        "            statum_graph::codebase::render::write_all_to_dir(&doc, &out_dir, &stem)?;\n",
    );
    source.push_str("        }\n");
    source.push_str("        \"suggest\" => {\n");
    source.push_str("            ensure_no_extra_args(&mut args, \"suggest\")?;\n");
    source.push_str("            let heuristic = cargo_statum_graph::collect_heuristic_overlay(\n");
    source.push_str("                &doc,\n");
    source.push_str("                &[\n");
    source.push_str(&inspect_package_sources_literal(&selections)?);
    source.push_str("                ],\n");
    source.push_str("            );\n");
    source.push_str("            print!(\n");
    source.push_str("                \"{}\",\n");
    source.push_str(
        "                cargo_statum_graph::render_composition_suggestions(&doc, &heuristic),\n",
    );
    source.push_str("            );\n");
    source.push_str("        }\n");
    source.push_str("        \"state-diagram\" => {\n");
    source.push_str("            let selection = take_string_arg(&mut args, \"state-diagram selection mode\")?;\n");
    source.push_str("            match selection.as_str() {\n");
    source.push_str("                \"auto\" => {\n");
    source.push_str("                    ensure_no_extra_args(&mut args, \"state-diagram\")?;\n");
    source.push_str("                    print!(\n");
    source.push_str("                        \"{}\",\n");
    source.push_str(
        "                        cargo_statum_graph::render_machine_state_diagram(&doc, None)?,\n",
    );
    source.push_str("                    );\n");
    source.push_str("                }\n");
    source.push_str("                \"machine\" => {\n");
    source.push_str(
        "                    let machine_selector = take_string_arg(&mut args, \"machine selector\")?;\n",
    );
    source.push_str("                    ensure_no_extra_args(&mut args, \"state-diagram\")?;\n");
    source.push_str("                    print!(\n");
    source.push_str("                        \"{}\",\n");
    source.push_str(
        "                        cargo_statum_graph::render_machine_state_diagram(&doc, Some(&machine_selector))?,\n",
    );
    source.push_str("                    );\n");
    source.push_str("                }\n");
    source.push_str("                other => {\n");
    source.push_str(
        "                    return Err(std::io::Error::other(format!(\"unknown state-diagram selection mode `{other}`\")).into());\n",
    );
    source.push_str("                }\n");
    source.push_str("            }\n");
    source.push_str("        }\n");
    source.push_str("        \"sequence-diagram\" => {\n");
    source.push_str("            let selection = take_string_arg(&mut args, \"sequence-diagram selection mode\")?;\n");
    source.push_str("            match selection.as_str() {\n");
    source.push_str("                \"auto\" => {\n");
    source
        .push_str("                    ensure_no_extra_args(&mut args, \"sequence-diagram\")?;\n");
    source.push_str("                    print!(\n");
    source.push_str("                        \"{}\",\n");
    source.push_str(
        "                        cargo_statum_graph::render_relation_sequence_diagram(&doc, cargo_statum_graph::RelationDiagramSelection::default())?,\n",
    );
    source.push_str("                    );\n");
    source.push_str("                }\n");
    source.push_str("                \"index\" => {\n");
    source.push_str(
        "                    let relation_index = take_string_arg(&mut args, \"relation index\")?;\n",
    );
    source
        .push_str("                    ensure_no_extra_args(&mut args, \"sequence-diagram\")?;\n");
    source.push_str(
        "                    let relation_index = relation_index.parse::<usize>().map_err(|source| std::io::Error::other(format!(\"invalid relation index `{relation_index}`: {source}\")))?;\n",
    );
    source.push_str("                    print!(\n");
    source.push_str("                        \"{}\",\n");
    source.push_str(
        "                        cargo_statum_graph::render_relation_sequence_diagram(&doc, cargo_statum_graph::RelationDiagramSelection {\n",
    );
    source.push_str("                            relation_index: Some(relation_index),\n");
    source.push_str("                            from_machine: None,\n");
    source.push_str("                            to_machine: None,\n");
    source.push_str("                        })?,\n");
    source.push_str("                    );\n");
    source.push_str("                }\n");
    source.push_str("                \"pair\" => {\n");
    source.push_str(
        "                    let from_machine = take_string_arg(&mut args, \"source machine selector\")?;\n",
    );
    source.push_str(
        "                    let to_machine = take_string_arg(&mut args, \"target machine selector\")?;\n",
    );
    source
        .push_str("                    ensure_no_extra_args(&mut args, \"sequence-diagram\")?;\n");
    source.push_str("                    print!(\n");
    source.push_str("                        \"{}\",\n");
    source.push_str(
        "                        cargo_statum_graph::render_relation_sequence_diagram(&doc, cargo_statum_graph::RelationDiagramSelection {\n",
    );
    source.push_str("                            relation_index: None,\n");
    source.push_str("                            from_machine: Some(&from_machine),\n");
    source.push_str("                            to_machine: Some(&to_machine),\n");
    source.push_str("                        })?,\n");
    source.push_str("                    );\n");
    source.push_str("                }\n");
    source.push_str("                other => {\n");
    source.push_str(
        "                    return Err(std::io::Error::other(format!(\"unknown sequence-diagram selection mode `{other}`\")).into());\n",
    );
    source.push_str("                }\n");
    source.push_str("            }\n");
    source.push_str("        }\n");
    source.push_str("        other => {\n");
    source.push_str(
        "            return Err(std::io::Error::other(format!(\"unknown runner command `{other}`\")).into());\n",
    );
    source.push_str("        }\n");
    source.push_str("    }\n");
    source.push_str("    Ok(())\n");
    source.push_str("}\n");
    source.push_str("\nfn take_os_arg(\n");
    source.push_str("    args: &mut impl Iterator<Item = OsString>,\n");
    source.push_str("    label: &str,\n");
    source.push_str(") -> Result<OsString, Box<dyn std::error::Error>> {\n");
    source.push_str("    args.next().ok_or_else(|| std::io::Error::other(format!(\"missing {label}\" )).into())\n");
    source.push_str("}\n");
    source.push_str("\nfn take_string_arg(\n");
    source.push_str("    args: &mut impl Iterator<Item = OsString>,\n");
    source.push_str("    label: &str,\n");
    source.push_str(") -> Result<String, Box<dyn std::error::Error>> {\n");
    source.push_str("    let value = take_os_arg(args, label)?;\n");
    source.push_str("    value.into_string().map_err(|value| {\n");
    source.push_str(
        "        std::io::Error::other(format!(\"{label} must be valid UTF-8: {:?}\", value)).into()\n",
    );
    source.push_str("    })\n");
    source.push_str("}\n");
    source.push_str("\nfn ensure_no_extra_args(\n");
    source.push_str("    args: &mut impl Iterator<Item = OsString>,\n");
    source.push_str("    command: &str,\n");
    source.push_str(") -> Result<(), Box<dyn std::error::Error>> {\n");
    source.push_str("    if let Some(extra) = args.next() {\n");
    source.push_str(
        "        Err(std::io::Error::other(format!(\"unexpected extra argument for {command}: {:?}\", extra)).into())\n",
    );
    source.push_str("    } else {\n");
    source.push_str("        Ok(())\n");
    source.push_str("    }\n");
    source.push_str("}\n");
    Ok(source)
}

fn inspect_package_sources_literal(selections: &[SelectedPackage]) -> Result<String, Error> {
    let mut literal = String::new();
    for selection in normalized_runner_selections(selections) {
        literal.push_str("            cargo_statum_graph::InspectPackageSource {\n");
        literal.push_str(&format!(
            "                package_name: {}.to_owned(),\n",
            rust_str(&selection.package_name)
        ));
        literal.push_str(&format!(
            "                manifest_dir: std::path::PathBuf::from({}),\n",
            rust_path(&selection.manifest_dir, "selected package manifest dir")?
        ));
        literal.push_str(&format!(
            "                lib_target_path: std::path::PathBuf::from({}),\n",
            rust_path(
                &selection.lib_target_path,
                "selected package library target"
            )?
        ));
        literal.push_str("            },\n");
    }
    Ok(literal)
}

fn normalized_runner_selections(selections: &[SelectedPackage]) -> Vec<SelectedPackage> {
    let mut normalized = selections.to_vec();
    normalized.sort_by(|left, right| {
        left.package_name
            .cmp(&right.package_name)
            .then_with(|| left.manifest_dir.cmp(&right.manifest_dir))
            .then_with(|| left.lib_target_path.cmp(&right.lib_target_path))
    });
    normalized
}

fn runner_key(selections: &[SelectedPackage], patch_root: Option<&Path>) -> Result<String, Error> {
    let mut canonical = format!("schema={RUNNER_SCHEMA_VERSION}\n");
    match patch_root {
        Some(root) => {
            canonical.push_str("patch=");
            canonical.push_str(path_utf8(root, "patched statum root")?);
            canonical.push('\n');
        }
        None => canonical.push_str("patch=<none>\n"),
    }
    for selection in normalized_runner_selections(selections) {
        canonical.push_str("package=");
        canonical.push_str(&selection.runner_key_fragment()?);
        canonical.push('\n');
    }

    Ok(format!(
        "v{RUNNER_SCHEMA_VERSION}-{:016x}",
        stable_runner_hash(&canonical)
    ))
}

fn stable_runner_hash(input: &str) -> u64 {
    const OFFSET: u64 = 0xcbf29ce484222325;
    const PRIME: u64 = 0x100000001b3;

    let mut hash = OFFSET;
    for byte in input.as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(PRIME);
    }
    hash
}

fn cached_runner_home(target_directory: &Path, runner_key: &str) -> PathBuf {
    target_directory
        .join("statum-graph")
        .join("runner")
        .join(runner_key)
}

fn write_file_if_changed(path: &Path, contents: &str, action: &'static str) -> Result<bool, Error> {
    if fs::read_to_string(path)
        .ok()
        .as_deref()
        .is_some_and(|existing| existing == contents)
    {
        return Ok(false);
    }

    fs::write(path, contents).map_err(|source| Error::Io {
        action,
        path: path.to_path_buf(),
        source,
    })?;
    Ok(true)
}

fn run_runner(
    runner: &CachedRunner,
    target_manifest_path: &Path,
    operation: &'static str,
    stdio: RunnerStdio,
    runtime_args: &[OsString],
) -> Result<(), Error> {
    match stdio {
        RunnerStdio::Captured => run_runner_captured(
            runner.manifest_path.clone(),
            &runner.target_directory,
            target_manifest_path,
            operation,
            runtime_args,
        )
        .map(|_| ()),
        RunnerStdio::Inherited => {
            let mut command = Command::new("cargo");
            command
                .arg("run")
                .arg("--quiet")
                .arg("--manifest-path")
                .arg(&runner.manifest_path)
                .arg("--target-dir")
                .arg(&runner.target_directory)
                .arg("--");
            command.args(runtime_args);

            let mut child = command
                .stdin(Stdio::inherit())
                .stdout(Stdio::inherit())
                .stderr(Stdio::piped())
                .spawn()
                .map_err(|source| Error::Io {
                    action: "run generated cargo runner",
                    path: runner.manifest_path.clone(),
                    source,
                })?;
            let stderr = child
                .stderr
                .take()
                .expect("piped stderr should be available");
            let reporter = thread::spawn(move || forward_and_capture_stderr(stderr));
            let status = child.wait().map_err(|source| Error::Io {
                action: "wait for generated cargo runner",
                path: runner.manifest_path.clone(),
                source,
            })?;
            let stderr = reporter
                .join()
                .unwrap_or_else(|_| Err(io::Error::other("stderr forwarder thread panicked")))
                .map_err(|source| Error::Io {
                    action: "capture generated cargo runner diagnostics",
                    path: runner.manifest_path.clone(),
                    source,
                })?;

            if status.success() {
                Ok(())
            } else {
                Err(Error::RunnerFailed {
                    operation,
                    manifest_path: target_manifest_path.to_path_buf(),
                    status,
                    details: normalize_runner_failure_details(&stderr, &[]),
                    diagnostics_reported: true,
                })
            }
        }
    }
}

fn run_runner_captured(
    runner_manifest_path: PathBuf,
    target_directory: &Path,
    target_manifest_path: &Path,
    operation: &'static str,
    runtime_args: &[OsString],
) -> Result<String, Error> {
    let mut command = Command::new("cargo");
    command
        .arg("run")
        .arg("--quiet")
        .arg("--manifest-path")
        .arg(&runner_manifest_path)
        .arg("--target-dir")
        .arg(target_directory)
        .arg("--");
    command.args(runtime_args);

    let output = command.output().map_err(|source| Error::Io {
        action: "run generated cargo runner",
        path: runner_manifest_path.clone(),
        source,
    })?;

    if output.status.success() {
        Ok(String::from_utf8_lossy(&output.stdout).into_owned())
    } else {
        Err(Error::RunnerFailed {
            operation,
            manifest_path: target_manifest_path.to_path_buf(),
            status: output.status,
            details: normalize_runner_failure_details(&output.stderr, &output.stdout),
            diagnostics_reported: false,
        })
    }
}

fn normalize_runner_failure_details(stderr: &[u8], stdout: &[u8]) -> Option<String> {
    let text = if stderr.is_empty() {
        String::from_utf8_lossy(stdout).into_owned()
    } else {
        String::from_utf8_lossy(stderr).into_owned()
    };
    let trimmed = text.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(
            trimmed
                .strip_prefix("Error: ")
                .unwrap_or(trimmed)
                .to_owned(),
        )
    }
}

fn runner_failure_details_look_like_build_failure(details: &str) -> bool {
    details.contains("could not compile `") || details.contains("error[E")
}

fn forward_and_capture_stderr<R: io::Read>(mut reader: R) -> io::Result<Vec<u8>> {
    let mut captured = Vec::new();
    let mut stderr = io::stderr().lock();
    let mut buffer = [0u8; 8192];
    loop {
        let read = reader.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        captured.extend_from_slice(&buffer[..read]);
        stderr.write_all(&buffer[..read])?;
    }
    stderr.flush()?;
    Ok(captured)
}

fn bundle_paths(out_dir: &Path, stem: &str) -> Vec<PathBuf> {
    GRAPH_EXTENSIONS
        .into_iter()
        .map(|extension| out_dir.join(format!("{stem}.{extension}")))
        .collect()
}

fn absolutize(path: &Path) -> io::Result<PathBuf> {
    let absolute = if path.is_absolute() {
        path.to_path_buf()
    } else {
        env::current_dir()?.join(path)
    };
    Ok(normalize_absolute_path(&absolute))
}

fn rust_path(value: &Path, role: &'static str) -> Result<String, Error> {
    Ok(rust_str(path_utf8(value, role)?))
}

fn toml_path(value: impl AsRef<Path>, role: &'static str) -> Result<String, Error> {
    Ok(toml_str(path_utf8(value.as_ref(), role)?))
}

fn rust_str(value: &str) -> String {
    let escaped: String = value.chars().flat_map(char::escape_default).collect();
    format!("\"{escaped}\"")
}

fn toml_str(value: &str) -> String {
    let mut escaped = String::with_capacity(value.len());
    for character in value.chars() {
        match character {
            '\\' => escaped.push_str("\\\\"),
            '"' => escaped.push_str("\\\""),
            '\u{08}' => escaped.push_str("\\b"),
            '\t' => escaped.push_str("\\t"),
            '\n' => escaped.push_str("\\n"),
            '\u{0C}' => escaped.push_str("\\f"),
            '\r' => escaped.push_str("\\r"),
            control if control.is_control() => {
                let code = control as u32;
                if code <= 0xFFFF {
                    write!(&mut escaped, "\\u{code:04X}")
                        .expect("writing to a String should not fail");
                } else {
                    write!(&mut escaped, "\\U{code:08X}")
                        .expect("writing to a String should not fail");
                }
            }
            other => escaped.push(other),
        }
    }

    format!("\"{escaped}\"")
}

fn validate_output_stem(stem: &str) -> Result<(), Error> {
    let mut components = Path::new(stem).components();
    match (components.next(), components.next()) {
        (Some(Component::Normal(_)), None) => Ok(()),
        _ => Err(Error::InvalidStem {
            stem: stem.to_owned(),
        }),
    }
}

fn path_utf8<'a>(path: &'a Path, role: &'static str) -> Result<&'a str, Error> {
    path.to_str().ok_or_else(|| Error::NonUtf8Path {
        role,
        path: path.to_path_buf(),
    })
}

fn normalize_absolute_path(path: &Path) -> PathBuf {
    debug_assert!(
        path.is_absolute(),
        "path should be absolute before normalization"
    );

    let mut normalized = PathBuf::new();
    for component in path.components() {
        match component {
            Component::Prefix(prefix) => normalized.push(prefix.as_os_str()),
            Component::RootDir => {
                normalized.push(std::path::MAIN_SEPARATOR.to_string());
            }
            Component::CurDir => {}
            Component::ParentDir => {
                if normalized.file_name().is_some() {
                    normalized.pop();
                }
            }
            Component::Normal(segment) => normalized.push(segment),
        }
    }

    normalized
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct SelectedPackage {
    package_name: String,
    manifest_dir: PathBuf,
    lib_target_path: PathBuf,
}

impl SelectedPackage {
    fn new(package: &Package, manifest_path: &Path) -> Result<Self, Error> {
        let Some(library_target) = library_target(package) else {
            return Err(Error::PackageHasNoLibrary {
                manifest_path: manifest_path.to_path_buf(),
                package: package.name.to_string(),
            });
        };

        Ok(Self {
            package_name: package.name.to_string(),
            manifest_dir: package
                .manifest_path
                .as_std_path()
                .parent()
                .expect("package manifest should have a parent")
                .to_path_buf(),
            lib_target_path: normalize_absolute_path(library_target.src_path.as_std_path()),
        })
    }

    fn dependency_alias(&self, index: usize) -> String {
        if self.package_name == GRAPH_PACKAGE_NAME {
            Self::graph_dependency_alias().to_owned()
        } else if self.package_name == HELPER_PACKAGE_NAME {
            Self::helper_dependency_alias().to_owned()
        } else {
            format!("graph_target_{index}")
        }
    }

    fn graph_dependency_alias() -> &'static str {
        "statum_graph"
    }

    fn helper_dependency_alias() -> &'static str {
        "cargo_statum_graph"
    }

    fn runner_key_fragment(&self) -> Result<String, Error> {
        Ok(format!(
            "{}|{}|{}",
            self.package_name,
            path_utf8(&self.manifest_dir, "selected package manifest dir")?,
            path_utf8(&self.lib_target_path, "selected package library target")?,
        ))
    }
}

struct PreparedRun {
    input: ResolvedInput,
    selections: Vec<SelectedPackage>,
    target_directory: PathBuf,
    patch_root: Option<PathBuf>,
}

#[derive(Clone, Copy)]
enum RunnerStdio {
    Captured,
    Inherited,
}

struct ResolvedInput {
    manifest_path: PathBuf,
    default_output_dir: PathBuf,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rust_str_escapes_control_characters() {
        assert_eq!(
            rust_str("line 1\n\"quoted\"\t\\tail"),
            "\"line 1\\n\\\"quoted\\\"\\t\\\\tail\""
        );
    }

    #[test]
    fn toml_str_escapes_control_characters() {
        assert_eq!(
            toml_str("line 1\n\"quoted\"\t\\tail\u{1F}"),
            "\"line 1\\n\\\"quoted\\\"\\t\\\\tail\\u001F\""
        );
    }

    #[cfg(unix)]
    #[test]
    fn rust_path_rejects_non_utf8_path() {
        use std::ffi::OsString;
        use std::os::unix::ffi::OsStringExt;

        let path = PathBuf::from(OsString::from_vec(vec![0x66, 0x80, 0x6F]));

        let error = rust_path(&path, "output directory").expect_err("non-UTF-8 path should fail");

        assert!(matches!(
            error,
            Error::NonUtf8Path {
                role: "output directory",
                ..
            }
        ));
    }

    #[cfg(unix)]
    #[test]
    fn toml_path_rejects_non_utf8_path() {
        use std::ffi::OsString;
        use std::os::unix::ffi::OsStringExt;

        let path = PathBuf::from(OsString::from_vec(vec![0x66, 0x80, 0x6F]));

        let error =
            toml_path(&path, "dependency package path").expect_err("non-UTF-8 path should fail");

        assert!(matches!(
            error,
            Error::NonUtf8Path {
                role: "dependency package path",
                ..
            }
        ));
    }

    #[test]
    fn absolutize_normalizes_cur_dir_components() {
        let current_dir = env::current_dir().expect("current dir");
        let normalized = absolutize(Path::new(".").join("Cargo.toml").as_path()).expect("path");

        assert_eq!(normalized, current_dir.join("Cargo.toml"));
    }

    #[test]
    fn runner_key_is_stable_across_selection_order() {
        let selections = vec![sample_selection("app"), sample_selection("domain")];
        let mut reversed = selections.clone();
        reversed.reverse();

        let left = runner_key(&selections, None).expect("runner key");
        let right = runner_key(&reversed, None).expect("runner key");

        assert_eq!(left, right);
    }

    #[test]
    fn runner_key_changes_for_different_package_sets_and_patch_roots() {
        let target_dir = tempfile::tempdir().expect("target tempdir");
        let all_packages = vec![sample_selection("app"), sample_selection("domain")];
        let app_only = vec![sample_selection("app")];
        let patch_a = PathBuf::from("/tmp/statum-a");
        let patch_b = PathBuf::from("/tmp/statum-b");

        let all_runner =
            materialize_cached_runner(target_dir.path(), &all_packages, Some(&patch_a))
                .expect("all-packages runner");
        let app_runner = materialize_cached_runner(target_dir.path(), &app_only, Some(&patch_a))
            .expect("app-only runner");
        let patch_runner =
            materialize_cached_runner(target_dir.path(), &all_packages, Some(&patch_b))
                .expect("patch-b runner");

        assert_ne!(all_runner.runner.key, app_runner.runner.key);
        assert_ne!(all_runner.runner.home_dir, app_runner.runner.home_dir);
        assert_ne!(all_runner.runner.key, patch_runner.runner.key);
        assert_ne!(all_runner.runner.home_dir, patch_runner.runner.home_dir);
    }

    #[test]
    fn post_diagnostics_note_explains_runner_compile_failure() {
        let error = Error::RunnerFailed {
            operation: "inspect session",
            manifest_path: PathBuf::from("/tmp/workspace/Cargo.toml"),
            status: exit_status_failure(),
            details: Some(
                "error[E0425]: cannot find function `missing_renderer` in this scope\nerror: could not compile `fixture-app` (lib) due to 1 previous error".to_owned(),
            ),
            diagnostics_reported: true,
        };

        let note = error
            .post_diagnostics_note()
            .expect("runner failures with reported diagnostics should explain next steps");

        assert!(note.contains("inspect session stopped while building the generated Statum runner"));
        assert!(note.contains("/tmp/workspace/Cargo.toml"));
        assert!(note.contains("must compile before cargo-statum-graph can continue"));
        assert!(note.contains("--package"));
    }

    #[test]
    fn post_diagnostics_note_is_absent_for_non_build_runner_diagnostics() {
        let error = Error::RunnerFailed {
            operation: "inspect session",
            manifest_path: PathBuf::from("/tmp/workspace/Cargo.toml"),
            status: exit_status_failure(),
            details: Some(
                "statum-graph inspect requires an interactive terminal on stdin and stdout."
                    .to_owned(),
            ),
            diagnostics_reported: true,
        };

        assert!(error.post_diagnostics_note().is_none());
    }

    #[test]
    fn detect_patch_root_from_target_workspace_finds_local_statum_checkout_dependency() {
        let temp = tempfile::tempdir().expect("fixture tempdir");
        let statum_root = temp.path().join("local-statum");
        write_fake_statum_workspace(&statum_root);

        let consumer_root = temp.path().join("consumer");
        fs::create_dir_all(consumer_root.join("src")).expect("consumer src dir");
        fs::write(
            consumer_root.join("Cargo.toml"),
            format!(
                "[package]\nname = \"consumer\"\nversion = \"0.1.0\"\nedition = \"2021\"\n\n[dependencies]\nstatum = {{ path = {:?} }}\n",
                statum_root.join("statum")
            ),
        )
        .expect("consumer manifest");
        fs::write(consumer_root.join("src/lib.rs"), "pub fn marker() {}\n").expect("consumer lib");

        let detected = detect_patch_root_from_target_workspace(&consumer_root.join("Cargo.toml"))
            .expect("patch root detection should succeed");

        assert_eq!(detected, Some(statum_root));
    }

    fn exit_status_failure() -> ExitStatus {
        #[cfg(unix)]
        {
            use std::os::unix::process::ExitStatusExt;

            ExitStatus::from_raw(1 << 8)
        }

        #[cfg(windows)]
        {
            use std::os::windows::process::ExitStatusExt;

            ExitStatus::from_raw(1)
        }
    }

    #[test]
    fn detect_patch_root_from_manifest_dirs_rejects_multiple_local_statum_roots() {
        let temp = tempfile::tempdir().expect("fixture tempdir");
        let root_a = temp.path().join("statum-a");
        let root_b = temp.path().join("statum-b");
        write_fake_statum_workspace(&root_a);
        write_fake_statum_workspace(&root_b);

        let error = detect_patch_root_from_manifest_dirs(
            Path::new("/tmp/consumer/Cargo.toml"),
            [root_a.join("statum"), root_b.join("statum")],
        )
        .expect_err("multiple local statum roots should fail closed");

        let Error::AmbiguousPatchStatumRoots { candidates, .. } = error else {
            panic!("expected ambiguous local statum root error");
        };
        assert_eq!(candidates, vec![root_a, root_b]);
    }

    #[test]
    fn build_runner_main_supports_generic_runtime_commands() {
        let selections = vec![sample_selection("app")];

        let source = build_runner_main(&selections).expect("runner source");

        assert!(source.contains("collect_heuristic_overlay"));
        assert!(source.contains("InspectPackageSource"));
        assert!(source.contains("cargo_statum_graph::run_inspector"));
        assert!(source.contains("is_terminal()"));
        assert!(source.contains("\"inspect\""));
        assert!(source.contains("\"export\" | \"codebase\""));
        assert!(source.contains("\"codebase\""));
        assert!(source.contains("\"suggest\""));
        assert!(source.contains("\"state-diagram\""));
        assert!(source.contains("\"sequence-diagram\""));
        assert!(source.contains("write_all_to_dir(&doc, &out_dir, &stem)?;"));
        assert!(source.contains("render_composition_suggestions(&doc, &heuristic)"));
        assert!(source.contains("render_machine_state_diagram(&doc, None)?"));
        assert!(source.contains("render_relation_sequence_diagram(&doc, cargo_statum_graph::RelationDiagramSelection::default())?"));
        assert!(source.contains("take_os_arg"));
        assert!(source.contains("ensure_no_extra_args"));
        assert!(!source.contains("/tmp/workspace/Cargo.toml"));
    }

    #[test]
    fn materialize_cached_runner_is_idempotent() {
        let target_dir = tempfile::tempdir().expect("target tempdir");
        let selections = vec![sample_selection("app")];

        let first =
            materialize_cached_runner(target_dir.path(), &selections, None).expect("first write");
        let second =
            materialize_cached_runner(target_dir.path(), &selections, None).expect("second write");

        assert!(first.manifest_rewritten);
        assert!(first.source_rewritten);
        assert!(!second.manifest_rewritten);
        assert!(!second.source_rewritten);
        assert_eq!(first.runner.home_dir, second.runner.home_dir);
        assert_eq!(first.runner.manifest_path, second.runner.manifest_path);
    }

    #[test]
    fn sequence_diagram_rejects_conflicting_relation_and_pair_selectors() {
        let error = sequence_diagram(SequenceDiagramOptions {
            input_path: PathBuf::from("/tmp/workspace"),
            package: None,
            relation: Some(3),
            from_machine: Some("workflow::Machine".to_owned()),
            to_machine: Some("task::Machine".to_owned()),
            patch_statum_root: None,
        })
        .expect_err("conflicting selectors should fail before runner setup");

        assert!(matches!(
            error,
            Error::InvalidDiagramSelection { ref message }
                if message == "choose either --relation or --from/--to, not both"
        ));
    }

    #[test]
    fn sequence_diagram_rejects_incomplete_machine_pair_selector() {
        let error = sequence_diagram(SequenceDiagramOptions {
            input_path: PathBuf::from("/tmp/workspace"),
            package: None,
            relation: None,
            from_machine: Some("workflow::Machine".to_owned()),
            to_machine: None,
            patch_statum_root: None,
        })
        .expect_err("incomplete pair selector should fail before runner setup");

        assert!(matches!(
            error,
            Error::InvalidDiagramSelection { ref message }
                if message == "relation pair selection requires both --from and --to"
        ));
    }

    #[test]
    fn build_runner_manifest_reuses_selected_helper_dependency() {
        let selections = vec![
            SelectedPackage {
                package_name: GRAPH_PACKAGE_NAME.to_owned(),
                manifest_dir: PathBuf::from("/tmp/graph"),
                lib_target_path: PathBuf::from("/tmp/graph/src/lib.rs"),
            },
            SelectedPackage {
                package_name: HELPER_PACKAGE_NAME.to_owned(),
                manifest_dir: PathBuf::from("/tmp/helper"),
                lib_target_path: PathBuf::from("/tmp/helper/src/lib.rs"),
            },
        ];

        let manifest = build_runner_manifest(&selections, None).expect("runner manifest");

        assert_eq!(manifest.matches("package = \"statum-graph\"").count(), 1);
        assert_eq!(
            manifest.matches("package = \"cargo-statum-graph\"").count(),
            1
        );
        assert!(manifest.contains("statum_graph = { package = \"statum-graph\""));
        assert!(manifest.contains("cargo_statum_graph = { package = \"cargo-statum-graph\""));
    }

    fn sample_selection(package_name: &str) -> SelectedPackage {
        SelectedPackage {
            package_name: package_name.to_owned(),
            manifest_dir: PathBuf::from(format!("/tmp/{package_name}")),
            lib_target_path: PathBuf::from(format!("/tmp/{package_name}/src/lib.rs")),
        }
    }

    fn write_fake_statum_workspace(root: &Path) {
        fs::create_dir_all(root).expect("fake statum root");
        fs::write(
            root.join("Cargo.toml"),
            "[workspace]\nresolver = \"2\"\nmembers = [\"macro_registry\", \"module_path_extractor\", \"statum\", \"statum-core\", \"statum-graph\", \"statum-macros\"]\n",
        )
        .expect("fake statum workspace manifest");

        for package in STATUM_WORKSPACE_PACKAGES {
            let package_dir = root.join(package);
            fs::create_dir_all(package_dir.join("src")).expect("fake package src dir");
            fs::write(
                package_dir.join("Cargo.toml"),
                format!(
                    "[package]\nname = \"{package}\"\nversion = \"0.7.0\"\nedition = \"2021\"\n"
                ),
            )
            .expect("fake package manifest");
            fs::write(package_dir.join("src/lib.rs"), "pub fn marker() {}\n")
                .expect("fake package lib");
        }
    }
}
