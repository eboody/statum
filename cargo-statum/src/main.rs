use std::{
    collections::{BTreeMap, BTreeSet},
    fmt,
    path::PathBuf,
    process::ExitCode,
};

use clap::{Parser, ValueEnum};
use statum_core::{StableGraphMetadata, StableStateMetadata, StableTransitionMetadata};
use statum_examples::showcases::axum_sqlite_review;

#[derive(Debug, Parser)]
#[command(
    bin_name = "cargo statum",
    about = "Statum workflow tooling prototypes",
    version
)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, clap::Subcommand)]
enum Command {
    /// Render or diff graph metadata for a supported Statum machine.
    Graph(GraphArgs),
    /// Render compact LLM-facing protocol context for a supported Statum machine.
    AgentContext(AgentContextArgs),
    /// Render human-readable workflow prose from graph metadata.
    Explain(ExplainArgs),
    /// Render reviewable protocol docs from one stable graph metadata value.
    Docs(DocsArgs),
}

#[derive(Debug, Parser)]
struct AgentContextArgs {
    /// Machine to render. Prototype support: axum-sqlite-review / DocumentMachine.
    #[arg(long)]
    machine: String,
}

#[derive(Debug, Parser)]
struct ExplainArgs {
    /// Machine to explain. Prototype support: axum-sqlite-review / DocumentMachine.
    #[arg(long)]
    machine: String,
}

#[derive(Debug, Parser)]
struct DocsArgs {
    /// Machine to document. Prototype support: axum-sqlite-review / DocumentMachine.
    #[arg(long)]
    machine: String,
}

#[derive(Debug, Parser)]
struct GraphArgs {
    /// Optional graph action. Use `diff` with --baseline and --current to compare snapshots.
    #[arg(value_enum)]
    action: Option<GraphAction>,
    /// Machine to render. Prototype support: axum-sqlite-review / DocumentMachine.
    #[arg(long)]
    machine: Option<String>,
    /// Baseline snapshot JSON for `graph diff`.
    #[arg(long)]
    baseline: Option<PathBuf>,
    /// Current snapshot JSON for `graph diff`.
    #[arg(long)]
    current: Option<PathBuf>,
    /// Output format.
    #[arg(long, value_enum, default_value_t = GraphFormat::Mermaid)]
    format: GraphFormat,
    /// Exit non-zero when the diff contains this severity or stronger.
    #[arg(long, value_enum)]
    fail_on: Option<Severity>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
enum GraphAction {
    Diff,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
enum GraphFormat {
    Mermaid,
    Dot,
    Json,
    Matrix,
    Lints,
    Markdown,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, ValueEnum, serde::Serialize)]
#[serde(rename_all = "snake_case")]
enum Severity {
    Informational,
    Review,
    Breaking,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum MachineSelector {
    AxumSqliteReviewDocument,
}

impl MachineSelector {
    fn parse(input: &str) -> Result<Self, MachineSelectorParseError> {
        match input {
            "axum-sqlite-review" | "DocumentMachine" | "axum_sqlite_review::DocumentMachine" => {
                Ok(Self::AxumSqliteReviewDocument)
            }
            _ => Err(MachineSelectorParseError {
                requested: input.to_owned(),
            }),
        }
    }
}

#[derive(Debug)]
struct MachineSelectorParseError {
    requested: String,
}

#[derive(Debug)]
enum Error {
    UnsupportedMachine(MachineSelectorParseError),
    UnsupportedFormat(&'static str),
    MissingArgument(&'static str),
    Io {
        path: PathBuf,
        source: std::io::Error,
    },
    Json {
        path: Option<PathBuf>,
        source: serde_json::Error,
    },
    InvalidSnapshot(String),
    InvalidComparison(String),
    FailOn(Severity),
}

impl fmt::Display for Error {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnsupportedMachine(error) => write!(
                formatter,
                "unsupported machine `{}`; supported machine selectors: axum-sqlite-review, DocumentMachine, axum_sqlite_review::DocumentMachine",
                error.requested
            ),
            Self::UnsupportedFormat(message) => formatter.write_str(message),
            Self::MissingArgument(argument) => {
                write!(formatter, "missing required argument `{argument}`")
            }
            Self::Io { path, source } => write!(
                formatter,
                "failed to read snapshot `{}`: {source}",
                path.display()
            ),
            Self::Json { path, source } => match path {
                Some(path) => write!(
                    formatter,
                    "failed to parse JSON snapshot `{}`: {source}",
                    path.display()
                ),
                None => write!(
                    formatter,
                    "failed to serialize graph diff as JSON: {source}"
                ),
            },
            Self::InvalidSnapshot(message) => formatter.write_str(message),
            Self::InvalidComparison(message) => formatter.write_str(message),
            Self::FailOn(severity) => write!(
                formatter,
                "graph diff contains {severity} changes and --fail-on requested failure"
            ),
        }
    }
}

impl fmt::Display for Severity {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Informational => "informational",
            Self::Review => "review",
            Self::Breaking => "breaking",
        })
    }
}

fn main() -> ExitCode {
    let args = normalize_cargo_subcommand_args(std::env::args_os());
    let cli = Cli::parse_from(args);

    match run(cli) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("error: {error}");
            ExitCode::FAILURE
        }
    }
}

fn run(cli: Cli) -> Result<(), Error> {
    match cli.command {
        Command::Graph(args) => run_graph(args),
        Command::AgentContext(args) => render_agent_context(args),
        Command::Explain(args) => render_explain(args),
        Command::Docs(args) => render_docs(args),
    }
}

fn render_agent_context(args: AgentContextArgs) -> Result<(), Error> {
    let _selector = MachineSelector::parse(&args.machine).map_err(Error::UnsupportedMachine)?;

    let metadata = axum_sqlite_review::workflow_stable_graph_metadata();
    print!("{}", render_agent_context_markdown(&metadata));
    Ok(())
}

fn render_explain(args: ExplainArgs) -> Result<(), Error> {
    let _selector = MachineSelector::parse(&args.machine).map_err(Error::UnsupportedMachine)?;

    let metadata = axum_sqlite_review::workflow_stable_graph_metadata();
    print!("{}", render_workflow_explanation(&metadata));
    Ok(())
}

fn render_docs(args: DocsArgs) -> Result<(), Error> {
    let _selector = MachineSelector::parse(&args.machine).map_err(Error::UnsupportedMachine)?;

    let metadata = axum_sqlite_review::workflow_stable_graph_metadata();
    print!(
        "{}",
        render_generated_protocol_docs(&args.machine, &metadata)
    );
    Ok(())
}

fn run_graph(args: GraphArgs) -> Result<(), Error> {
    match args.action {
        Some(GraphAction::Diff) => render_graph_diff(args),
        None => render_graph(args),
    }
}

fn render_graph(args: GraphArgs) -> Result<(), Error> {
    let machine = args.machine.ok_or(Error::MissingArgument("--machine"))?;
    let _selector = MachineSelector::parse(&machine).map_err(Error::UnsupportedMachine)?;

    let metadata = axum_sqlite_review::workflow_stable_graph_metadata();
    match args.format {
        GraphFormat::Mermaid => print!("{}", metadata.to_mermaid_state_diagram()),
        GraphFormat::Dot => print!("{}", metadata.to_dot_graph()),
        GraphFormat::Matrix => print!("{}", metadata.to_transition_matrix_table()),
        GraphFormat::Lints => print!("{}", metadata.to_graph_lint_report()),
        GraphFormat::Json => {
            serde_json::to_writer_pretty(std::io::stdout(), &metadata)
                .map_err(|source| Error::Json { path: None, source })?;
            println!();
        }
        GraphFormat::Markdown => {
            return Err(Error::UnsupportedFormat(
                "markdown output is supported for `cargo statum graph diff`, not graph rendering",
            ));
        }
    }

    Ok(())
}

#[derive(Debug, serde::Deserialize)]
struct GraphSnapshot {
    snapshot_version: String,
    package: String,
    machine_key: String,
    feature_set: serde_json::Value,
    graph: StableGraphMetadata,
}

#[derive(Debug, serde::Serialize)]
struct GraphDiffReport {
    diff_version: &'static str,
    machine_key: String,
    summary: DiffSummary,
    changes: Vec<GraphChange>,
    authority: DiffAuthority,
}

#[derive(Default, Debug, serde::Serialize)]
struct DiffSummary {
    breaking: usize,
    review: usize,
    informational: usize,
}

#[derive(Debug, serde::Serialize)]
struct GraphChange {
    severity: Severity,
    category: &'static str,
    key: String,
    message: String,
}

#[derive(Debug, serde::Serialize)]
struct DiffAuthority {
    before: String,
    after: String,
    observation_point: &'static str,
}

fn render_graph_diff(args: GraphArgs) -> Result<(), Error> {
    let baseline = args.baseline.ok_or(Error::MissingArgument("--baseline"))?;
    let current = args.current.ok_or(Error::MissingArgument("--current"))?;
    let before = read_snapshot(&baseline)?;
    let after = read_snapshot(&current)?;
    let report = diff_snapshots(&before, &after)?;

    match args.format {
        GraphFormat::Json => {
            serde_json::to_writer_pretty(std::io::stdout(), &report)
                .map_err(|source| Error::Json { path: None, source })?;
            println!();
        }
        GraphFormat::Markdown => print!("{}", render_diff_markdown(&report)),
        GraphFormat::Mermaid | GraphFormat::Dot | GraphFormat::Matrix | GraphFormat::Lints => {
            return Err(Error::UnsupportedFormat(
                "graph diff supports --format json or --format markdown",
            ));
        }
    }

    if args.fail_on.is_some_and(|fail_on| {
        report
            .changes
            .iter()
            .any(|change| change.severity >= fail_on)
    }) {
        return Err(Error::FailOn(args.fail_on.expect("checked above")));
    }

    Ok(())
}

fn render_workflow_explanation(metadata: &StableGraphMetadata) -> String {
    let mut output = String::new();
    output.push_str("# Workflow explanation: ");
    output.push_str(&plain_field(&metadata.machine.rust_type_path));
    output.push_str("\n\n");
    output.push_str(&render_workflow_explanation_sections(metadata, "##"));
    output
}

fn render_workflow_explanation_sections(metadata: &StableGraphMetadata, heading: &str) -> String {
    let mut output = String::new();
    let authority = authority_value(metadata);
    output.push_str("Observation point: StableGraphMetadata ");
    output.push_str(&version_value(metadata));
    output.push_str(" (`");
    output.push_str(&authority);
    output.push_str("`).\n");
    output.push_str("This explanation is derived from metadata only; it does not inspect source code, macro expansion, type checking, runtime policy, validators, storage rows, or side effects.\n\n");

    output.push_str(heading);
    output.push_str(" Human workflow\n");
    let transitions_by_state = transitions_by_source(metadata);
    let mut step = 1;
    for state in &metadata.states {
        let state_name = plain_field(&state.rust_name);
        if let Some(transitions) = transitions_by_state.get(&state.rust_name) {
            for transition in transitions {
                output.push_str(&step.to_string());
                output.push_str(". ");
                output.push_str(&state_name);
                output.push_str(" can move to ");
                output.push_str(&join_plain(&transition.to_states, " or "));
                output.push_str(" by calling `");
                output.push_str(&plain_field(&transition.method_name));
                output.push_str("`.\n");
                step += 1;
            }
        } else {
            output.push_str("- ");
            output.push_str(&state_name);
            output.push_str(" has no outgoing transitions in this metadata.\n");
        }
    }

    output.push('\n');
    output.push_str(heading);
    output.push_str(" State data notes\n");
    for state in &metadata.states {
        output.push_str("- ");
        output.push_str(&plain_field(&state.rust_name));
        output.push_str(if state.has_data {
            " carries state data according to the metadata."
        } else {
            " is a marker state according to the metadata."
        });
        output.push('\n');
    }

    output.push('\n');
    output.push_str(heading);
    output.push_str(" Review checklist\n");
    output.push_str("- Confirm product language, permissions, side effects, and persistence behavior outside this generated explanation.\n");
    output.push_str("- Treat validators as application/runtime concerns; StableGraphMetadata v1 does not include validator rules.\n");
    if metadata.unsupported_cases.is_empty() {
        output.push_str("- Unsupported-case metadata: none reported.\n");
    } else {
        output.push_str("- Unsupported-case metadata: ");
        let unsupported = metadata
            .unsupported_cases
            .iter()
            .map(|case| unsupported_case_value(*case))
            .collect::<Vec<_>>();
        output.push_str(&join_plain(&unsupported, ", "));
        output.push_str(".\n");
    }

    output
}

fn render_generated_protocol_docs(machine_arg: &str, metadata: &StableGraphMetadata) -> String {
    let mut output = String::new();
    output.push_str("# Generated protocol docs: ");
    output.push_str(&plain_field(&metadata.machine.rust_type_path));
    output.push_str("\n\n");
    output.push_str(
        "Generated from one `StableGraphMetadata` value. Observation point: StableGraphMetadata ",
    );
    output.push_str(&version_value(metadata));
    output.push_str(" (`");
    output.push_str(&authority_value(metadata));
    output.push_str("`). The sections below are rendered in one CLI invocation without re-scanning source code, macro expansion, type checking, runtime policy, validators, persisted rows, or side effects.\n\n");

    output.push_str("## Mermaid state diagram\n\n");
    output.push_str("```mermaid\n");
    output.push_str(&metadata.to_mermaid_state_diagram());
    output.push_str("```\n\n");

    output.push_str("## Transition table\n\n");
    output.push_str(&metadata.to_transition_matrix_table());
    output.push('\n');

    output.push_str("## Narrative summary\n\n");
    output.push_str(&render_workflow_explanation_sections(metadata, "###"));
    output.push('\n');

    output.push_str("## Keeping generated artifacts current\n\n");
    output.push_str("- Treat this page as generated output from StableGraphMetadata, not hand-maintained workflow prose.\n");
    output.push_str("- After changing states, transitions, labels, or metadata authority, re-run `cargo statum docs --machine ");
    output.push_str(&plain_field(machine_arg));
    output.push_str("` and review the full diff.\n");
    output.push_str("- Commit the generated docs update with the workflow code change. Because all sections are rendered from the same metadata value, regenerated artifacts avoid hand-maintained disagreement; review which sections changed and whether that matches the metadata change.\n");
    output.push_str("- If unsupported-case or authority metadata changes, review docs/introspection-authority.md before publishing the regenerated artifact.\n");

    output
}

fn transitions_by_source(
    metadata: &StableGraphMetadata,
) -> BTreeMap<String, Vec<&StableTransitionMetadata>> {
    let mut by_state: BTreeMap<String, Vec<&StableTransitionMetadata>> = BTreeMap::new();
    for transition in &metadata.transitions {
        by_state
            .entry(transition.from_state.clone())
            .or_default()
            .push(transition);
    }
    by_state
}

fn render_agent_context_markdown(metadata: &StableGraphMetadata) -> String {
    let mut output = String::new();
    output.push_str("# statum agent-context v1\n");
    output.push_str("machine: ");
    output.push_str(&plain_field(&metadata.machine.rust_type_path));
    output.push('\n');
    output.push_str("generated_from: StableGraphMetadata v1\n");
    output.push_str("authority: ");
    output.push_str(&authority_value(metadata));
    output.push_str("\n\n");

    output.push_str("states:\n");
    for state in &metadata.states {
        output.push_str("- ");
        output.push_str(&plain_field(&state.rust_name));
        output.push_str(" data=");
        output.push_str(if state.has_data { "yes" } else { "no" });
        if let Some(label) = &state.label {
            output.push_str(" label=\"");
            output.push_str(&plain_field(label));
            output.push('"');
        }
        output.push('\n');
    }

    output.push_str("\nlegal_transitions:\n");
    if metadata.transitions.is_empty() {
        output.push_str("- none\n");
    } else {
        for transition in &metadata.transitions {
            output.push_str("- ");
            output.push_str(&plain_field(&transition.from_state));
            output.push('.');
            output.push_str(&plain_field(&transition.method_name));
            output.push_str(" -> ");
            output.push_str(&join_plain(&transition.to_states, " | "));
            output.push('\n');
        }
    }

    output.push_str("\nforbidden_calls:\n");
    let all_calls = all_transition_methods(metadata);
    if all_calls.is_empty() {
        output.push_str("- none\n");
    } else {
        for state in &metadata.states {
            let legal_from_state = metadata
                .transitions
                .iter()
                .filter(|transition| transition.from_state == state.rust_name)
                .map(|transition| transition.method_name.as_str())
                .collect::<BTreeSet<_>>();
            let forbidden = all_calls
                .iter()
                .filter(|method| !legal_from_state.contains(method.as_str()))
                .cloned()
                .collect::<Vec<_>>();
            output.push_str("- ");
            output.push_str(&plain_field(&state.rust_name));
            output.push_str(": ");
            if forbidden.is_empty() {
                output.push_str("none");
            } else {
                output.push_str(&join_plain(&forbidden, ", "));
            }
            output.push('\n');
        }
    }

    output.push_str("\nrehydration_rules:\n");
    output.push_str("- match persisted state name to one listed under states\n");
    output.push_str("- reject persisted state names absent from this metadata document\n");
    for state in &metadata.states {
        output.push_str("- ");
        output.push_str(&plain_field(&state.rust_name));
        output.push_str(": ");
        output.push_str(if state.has_data {
            "persisted state data required"
        } else {
            "marker state; no persisted state data expected"
        });
        output.push('\n');
    }

    output.push_str("\nvalidators:\n");
    output.push_str("- not in StableGraphMetadata v1; inspect runtime/application validators before executing side effects\n");

    output.push_str("\ncaveats:\n");
    output.push_str("- generated only from StableGraphMetadata; no source re-scan, macro expansion, type checking, runtime policy, or persisted-state inspection\n");
    for unsupported_case in &metadata.unsupported_cases {
        output.push_str("- unsupported: ");
        output.push_str(&unsupported_case_value(*unsupported_case));
        output.push('\n');
    }

    output
}

fn all_transition_methods(metadata: &StableGraphMetadata) -> Vec<String> {
    metadata
        .transitions
        .iter()
        .map(|transition| transition.method_name.clone())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

fn join_plain(values: &[String], separator: &str) -> String {
    values
        .iter()
        .map(|value| plain_field(value))
        .collect::<Vec<_>>()
        .join(separator)
}

fn unsupported_case_value(case: statum_core::UnsupportedGraphMetadataCase) -> String {
    serde_json::to_value(case)
        .ok()
        .and_then(|value| value.as_str().map(ToOwned::to_owned))
        .unwrap_or_else(|| "unknown".to_string())
}

fn plain_field(value: &str) -> String {
    value.replace(['\n', '\r', '\t'], " ")
}

fn read_snapshot(path: &PathBuf) -> Result<GraphSnapshot, Error> {
    let bytes = std::fs::read(path).map_err(|source| Error::Io {
        path: path.clone(),
        source,
    })?;
    serde_json::from_slice(&bytes).map_err(|source| Error::Json {
        path: Some(path.clone()),
        source,
    })
}

fn diff_snapshots(before: &GraphSnapshot, after: &GraphSnapshot) -> Result<GraphDiffReport, Error> {
    validate_snapshot_pair(before, after)?;
    let before_states = index_states(before)?;
    let after_states = index_states(after)?;
    let before_sites = index_transition_sites(before)?;
    let after_sites = index_transition_sites(after)?;
    let before_edges = edge_keys(&before.graph.transitions);
    let after_edges = edge_keys(&after.graph.transitions);

    let mut changes = Vec::new();

    for state in before_states
        .keys()
        .filter(|state| !after_states.contains_key(*state))
    {
        push_change(
            &mut changes,
            Severity::Breaking,
            "state_removed",
            (*state).clone(),
            format!(
                "removed state `{state}`; persisted rows/events may need migration before this graph ships"
            ),
        );
    }
    for state in after_states
        .keys()
        .filter(|state| !before_states.contains_key(*state))
    {
        push_change(
            &mut changes,
            Severity::Review,
            "state_added",
            (*state).clone(),
            format!(
                "added state `{state}`; review persistence, authorization, fixtures, and dashboards"
            ),
        );
    }

    for site in before_sites
        .keys()
        .filter(|site| !after_sites.contains_key(*site))
    {
        push_change(
            &mut changes,
            Severity::Breaking,
            "transition_removed",
            site.clone(),
            format!(
                "removed transition `{site}`; callers, event replay, or recorded transition ids may need migration"
            ),
        );
    }
    for site in after_sites
        .keys()
        .filter(|site| !before_sites.contains_key(*site))
    {
        push_change(
            &mut changes,
            Severity::Review,
            "transition_added",
            site.clone(),
            format!(
                "added transition `{site}`; review authorization, side effects, and expected events"
            ),
        );
    }

    for edge in before_edges
        .iter()
        .filter(|edge| !after_edges.contains(*edge))
    {
        let key = edge_key(edge);
        push_change(
            &mut changes,
            Severity::Breaking,
            "edge_removed",
            key.clone(),
            format!(
                "removed edge `{key}`; persisted events may no longer replay and users may lose a legal path"
            ),
        );
    }
    for edge in after_edges
        .iter()
        .filter(|edge| !before_edges.contains(*edge))
    {
        let key = edge_key(edge);
        push_change(
            &mut changes,
            Severity::Review,
            "edge_added",
            key.clone(),
            format!(
                "added edge `{key}`; review product, security, and data-model migration concerns"
            ),
        );
    }

    for (state, before_state) in &before_states {
        let Some(after_state) = after_states.get(state) else {
            continue;
        };
        if before_state.has_data != after_state.has_data {
            push_change(
                &mut changes,
                Severity::Breaking,
                "state_data_presence_changed",
                state.clone(),
                format!(
                    "state `{state}` changed data presence; field-level migration safety is outside StableGraphMetadata v1"
                ),
            );
        }
        if before_state.label != after_state.label
            || before_state.description != after_state.description
        {
            push_change(
                &mut changes,
                Severity::Informational,
                "state_presentation_changed",
                state.clone(),
                format!(
                    "state `{state}` label or description changed; workflow legality is unchanged"
                ),
            );
        }
    }

    if before.graph.authority != after.graph.authority {
        push_change(
            &mut changes,
            Severity::Review,
            "authority_changed",
            after.machine_key.clone(),
            format!(
                "metadata authority changed from `{}` to `{}`; do not treat the authority surface as unchanged",
                serde_json::to_value(before.graph.authority)
                    .unwrap_or_default()
                    .as_str()
                    .unwrap_or("unknown"),
                serde_json::to_value(after.graph.authority)
                    .unwrap_or_default()
                    .as_str()
                    .unwrap_or("unknown")
            ),
        );
    }
    if before.graph.unsupported_cases != after.graph.unsupported_cases {
        push_change(
            &mut changes,
            Severity::Review,
            "unsupported_cases_changed",
            after.machine_key.clone(),
            "unsupported-case list changed; review whether the snapshot now represents a narrower or wider authority surface".to_string(),
        );
    }

    changes.sort_by(|left, right| {
        right
            .severity
            .cmp(&left.severity)
            .then(left.category.cmp(right.category))
            .then(left.key.cmp(&right.key))
    });

    let summary = summarize_changes(&changes);
    Ok(GraphDiffReport {
        diff_version: "v1",
        machine_key: after.machine_key.clone(),
        summary,
        changes,
        authority: DiffAuthority {
            before: authority_value(&before.graph),
            after: authority_value(&after.graph),
            observation_point: "serialized_stable_graph_metadata",
        },
    })
}

fn validate_snapshot_pair(before: &GraphSnapshot, after: &GraphSnapshot) -> Result<(), Error> {
    if before.snapshot_version != "v1" || after.snapshot_version != "v1" {
        return Err(Error::InvalidSnapshot(
            "unsupported graph snapshot wrapper version; prototype supports snapshot_version `v1`"
                .to_string(),
        ));
    }
    if before.package != after.package {
        return Err(Error::InvalidComparison(format!(
            "cannot diff snapshots from different packages: `{}` vs `{}`",
            before.package, after.package
        )));
    }
    if before.machine_key != after.machine_key {
        return Err(Error::InvalidComparison(format!(
            "cannot diff different machine keys: `{}` vs `{}`",
            before.machine_key, after.machine_key
        )));
    }
    if before.feature_set != after.feature_set {
        return Err(Error::InvalidComparison(
            "cannot diff snapshots with different feature_set values".to_string(),
        ));
    }
    if before.graph.version != after.graph.version {
        return Err(Error::InvalidComparison(
            "cannot diff different stable graph metadata versions without an adapter".to_string(),
        ));
    }
    Ok(())
}

fn index_states(snapshot: &GraphSnapshot) -> Result<BTreeMap<String, &StableStateMetadata>, Error> {
    let mut states = BTreeMap::new();
    for state in &snapshot.graph.states {
        if states.insert(state.rust_name.clone(), state).is_some() {
            return Err(Error::InvalidSnapshot(format!(
                "duplicate state `{}` in snapshot `{}`",
                state.rust_name, snapshot.machine_key
            )));
        }
    }
    Ok(states)
}

fn index_transition_sites(
    snapshot: &GraphSnapshot,
) -> Result<BTreeMap<String, &StableTransitionMetadata>, Error> {
    let mut transitions = BTreeMap::new();
    for transition in &snapshot.graph.transitions {
        let key = transition_site_key(transition);
        if transitions.insert(key.clone(), transition).is_some() {
            return Err(Error::InvalidSnapshot(format!(
                "duplicate transition site `{key}` in snapshot `{}`",
                snapshot.machine_key
            )));
        }
    }
    Ok(transitions)
}

fn edge_keys(transitions: &[StableTransitionMetadata]) -> BTreeSet<(String, String, String)> {
    let mut edges = BTreeSet::new();
    for transition in transitions {
        for target in &transition.to_states {
            edges.insert((
                transition.from_state.clone(),
                transition.method_name.clone(),
                target.clone(),
            ));
        }
    }
    edges
}

fn transition_site_key(transition: &StableTransitionMetadata) -> String {
    format!("{}::{}", transition.from_state, transition.method_name)
}

fn edge_key(edge: &(String, String, String)) -> String {
    format!("{}::{}->{}", edge.0, edge.1, edge.2)
}

fn push_change(
    changes: &mut Vec<GraphChange>,
    severity: Severity,
    category: &'static str,
    key: String,
    message: String,
) {
    changes.push(GraphChange {
        severity,
        category,
        key,
        message,
    });
}

fn summarize_changes(changes: &[GraphChange]) -> DiffSummary {
    let mut summary = DiffSummary::default();
    for change in changes {
        match change.severity {
            Severity::Breaking => summary.breaking += 1,
            Severity::Review => summary.review += 1,
            Severity::Informational => summary.informational += 1,
        }
    }
    summary
}

fn authority_value(graph: &StableGraphMetadata) -> String {
    serde_json::to_value(graph.authority)
        .ok()
        .and_then(|value| value.as_str().map(ToOwned::to_owned))
        .unwrap_or_else(|| "unknown".to_string())
}

fn version_value(graph: &StableGraphMetadata) -> String {
    serde_json::to_value(graph.version)
        .ok()
        .and_then(|value| value.as_str().map(ToOwned::to_owned))
        .unwrap_or_else(|| "unknown".to_string())
}

fn render_diff_markdown(report: &GraphDiffReport) -> String {
    let mut output = String::new();
    output.push_str("### Statum graph diff: `");
    output.push_str(&report.machine_key);
    output.push_str("`\n\n");
    output.push_str("Authority: serialized `StableGraphMetadata` (`");
    output.push_str(&report.authority.before);
    output.push_str("` before, `");
    output.push_str(&report.authority.after);
    output.push_str("` after). This is a workflow-graph diff, not a full Rust behavior diff.\n\n");
    output.push_str("| severity | change | migration concern |\n");
    output.push_str("| --- | --- | --- |\n");

    if report.changes.is_empty() {
        output.push_str("| informational | no graph changes | no migration concerns detected |\n");
    } else {
        for change in &report.changes {
            output.push_str("| ");
            output.push_str(&change.severity.to_string());
            output.push_str(" | ");
            output.push_str(&markdown_change_label(change));
            output.push_str(" | ");
            output.push_str(&change.message);
            output.push_str(" |\n");
        }
    }

    output
}

fn markdown_change_label(change: &GraphChange) -> String {
    match change.category {
        "state_removed" => format!("removed state `{}`", change.key),
        "state_added" => format!("added state `{}`", change.key),
        "transition_removed" => format!("removed transition `{}`", change.key),
        "transition_added" => format!("added transition `{}`", change.key),
        "edge_removed" => format!("removed edge `{}`", change.key),
        "edge_added" => format!("added edge `{}`", change.key),
        "state_data_presence_changed" => format!("state data changed `{}`", change.key),
        "state_presentation_changed" => format!("state presentation changed `{}`", change.key),
        "authority_changed" => "authority changed".to_string(),
        "unsupported_cases_changed" => "unsupported cases changed".to_string(),
        _ => format!("{} `{}`", change.category, change.key),
    }
}

fn normalize_cargo_subcommand_args<I>(args: I) -> Vec<std::ffi::OsString>
where
    I: IntoIterator<Item = std::ffi::OsString>,
{
    let mut args = args.into_iter().collect::<Vec<_>>();
    if args.get(1).is_some_and(|arg| arg == "statum") {
        args.remove(1);
    }
    args
}
