use std::ffi::OsString;
use std::path::PathBuf;
use std::process::ExitCode;

use clap::{Args, Parser};

use cargo_statum_graph::{
    export, inspect, sequence_diagram, state_diagram, suggest, ExportOptions, InspectOptions,
    SequenceDiagramOptions, StateDiagramOptions, SuggestOptions,
};

#[derive(Debug, Parser)]
#[command(name = "cargo-statum-graph")]
#[command(
    about = "Export exact Statum workspace bundles and launch Statum Inspector for existing crates"
)]
enum Cli {
    #[command(name = "export", visible_alias = "codebase")]
    Export(ExportArgs),
    #[command(name = "inspect")]
    Inspect(InspectArgs),
    #[command(name = "state-diagram")]
    StateDiagram(StateDiagramArgs),
    #[command(name = "sequence-diagram")]
    SequenceDiagram(SequenceDiagramArgs),
    #[command(name = "suggest")]
    Suggest(SuggestArgs),
}

#[derive(Debug, Args)]
struct ExportArgs {
    #[arg(value_name = "PATH", default_value = ".")]
    path: PathBuf,
    #[arg(long)]
    manifest_path: Option<PathBuf>,
    #[arg(long)]
    package: Option<String>,
    #[arg(long)]
    out_dir: Option<PathBuf>,
    #[arg(long, default_value = "codebase")]
    stem: String,
    #[arg(long)]
    patch_statum_root: Option<PathBuf>,
}

#[derive(Debug, Args)]
struct InspectArgs {
    #[arg(value_name = "PATH", default_value = ".")]
    path: PathBuf,
    #[arg(long)]
    manifest_path: Option<PathBuf>,
    #[arg(long)]
    package: Option<String>,
    #[arg(long)]
    patch_statum_root: Option<PathBuf>,
}

#[derive(Debug, Args)]
struct SuggestArgs {
    #[arg(value_name = "PATH", default_value = ".")]
    path: PathBuf,
    #[arg(long)]
    manifest_path: Option<PathBuf>,
    #[arg(long)]
    package: Option<String>,
    #[arg(long)]
    patch_statum_root: Option<PathBuf>,
}

#[derive(Debug, Args)]
struct StateDiagramArgs {
    #[arg(value_name = "PATH", default_value = ".")]
    path: PathBuf,
    #[arg(long)]
    manifest_path: Option<PathBuf>,
    #[arg(long)]
    package: Option<String>,
    #[arg(long)]
    machine: Option<String>,
    #[arg(long)]
    patch_statum_root: Option<PathBuf>,
}

#[derive(Debug, Args)]
struct SequenceDiagramArgs {
    #[arg(value_name = "PATH", default_value = ".")]
    path: PathBuf,
    #[arg(long)]
    manifest_path: Option<PathBuf>,
    #[arg(long)]
    package: Option<String>,
    #[arg(long, conflicts_with_all = ["from_machine", "to_machine"])]
    relation: Option<usize>,
    #[arg(long = "from", requires = "to_machine", conflicts_with = "relation")]
    from_machine: Option<String>,
    #[arg(long = "to", requires = "from_machine", conflicts_with = "relation")]
    to_machine: Option<String>,
    #[arg(long)]
    patch_statum_root: Option<PathBuf>,
}

fn main() -> ExitCode {
    match run_main() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            if !error.diagnostics_reported() {
                eprintln!("{error}");
            }
            ExitCode::FAILURE
        }
    }
}

fn run_main() -> Result<(), cargo_statum_graph::Error> {
    let cli = parse_cli_from(std::env::args_os());
    let output = match cli {
        Cli::Export(args) => export(ExportOptions {
            input_path: args.manifest_path.unwrap_or(args.path),
            package: args.package,
            out_dir: args.out_dir,
            stem: args.stem,
            patch_statum_root: args.patch_statum_root,
        })?
        .into_iter()
        .map(|path| path.display().to_string())
        .collect::<Vec<_>>(),
        Cli::Inspect(args) => {
            inspect(InspectOptions {
                input_path: args.manifest_path.unwrap_or(args.path),
                package: args.package,
                patch_statum_root: args.patch_statum_root,
            })?;
            Vec::new()
        }
        Cli::StateDiagram(args) => state_diagram(StateDiagramOptions {
            input_path: args.manifest_path.unwrap_or(args.path),
            package: args.package,
            machine: args.machine,
            patch_statum_root: args.patch_statum_root,
        })?
        .lines()
        .map(str::to_owned)
        .collect(),
        Cli::SequenceDiagram(args) => sequence_diagram(SequenceDiagramOptions {
            input_path: args.manifest_path.unwrap_or(args.path),
            package: args.package,
            relation: args.relation,
            from_machine: args.from_machine,
            to_machine: args.to_machine,
            patch_statum_root: args.patch_statum_root,
        })?
        .lines()
        .map(str::to_owned)
        .collect(),
        Cli::Suggest(args) => suggest(SuggestOptions {
            input_path: args.manifest_path.unwrap_or(args.path),
            package: args.package,
            patch_statum_root: args.patch_statum_root,
        })?
        .lines()
        .map(str::to_owned)
        .collect(),
    };

    for line in output {
        println!("{line}");
    }

    Ok(())
}

fn parse_cli_from<I, T>(args: I) -> Cli
where
    I: IntoIterator<Item = T>,
    T: Into<OsString>,
{
    let mut args = args.into_iter().map(Into::into).collect::<Vec<_>>();
    if args.get(1).is_some_and(|arg| arg == "statum-graph") {
        args.remove(1);
    }

    Cli::parse_from(args)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_cli_from_accepts_export_subcommand() {
        let cli = parse_cli_from(["cargo-statum-graph", "export", "/tmp/workspace"]);

        let Cli::Export(args) = cli else {
            panic!("expected export subcommand");
        };
        assert_eq!(args.path, PathBuf::from("/tmp/workspace"));
    }

    #[test]
    fn parse_cli_from_accepts_cargo_injected_subcommand_name() {
        let cli = parse_cli_from([
            "cargo-statum-graph",
            "statum-graph",
            "export",
            "/tmp/workspace",
        ]);

        let Cli::Export(args) = cli else {
            panic!("expected export subcommand");
        };
        assert_eq!(args.path, PathBuf::from("/tmp/workspace"));
    }

    #[test]
    fn parse_cli_from_accepts_legacy_codebase_alias() {
        let cli = parse_cli_from(["cargo-statum-graph", "codebase", "/tmp/workspace"]);

        let Cli::Export(args) = cli else {
            panic!("expected export subcommand");
        };
        assert_eq!(args.path, PathBuf::from("/tmp/workspace"));
    }

    #[test]
    fn parse_cli_from_accepts_cargo_injected_legacy_codebase_alias() {
        let cli = parse_cli_from([
            "cargo-statum-graph",
            "statum-graph",
            "codebase",
            "/tmp/workspace",
        ]);

        let Cli::Export(args) = cli else {
            panic!("expected export subcommand");
        };
        assert_eq!(args.path, PathBuf::from("/tmp/workspace"));
    }

    #[test]
    fn parse_cli_from_accepts_inspect_subcommand() {
        let cli = parse_cli_from(["cargo-statum-graph", "inspect", "/tmp/workspace"]);

        let Cli::Inspect(args) = cli else {
            panic!("expected inspect subcommand");
        };
        assert_eq!(args.path, PathBuf::from("/tmp/workspace"));
    }

    #[test]
    fn parse_cli_from_accepts_suggest_subcommand() {
        let cli = parse_cli_from(["cargo-statum-graph", "suggest", "/tmp/workspace"]);

        let Cli::Suggest(args) = cli else {
            panic!("expected suggest subcommand");
        };
        assert_eq!(args.path, PathBuf::from("/tmp/workspace"));
    }

    #[test]
    fn parse_cli_from_accepts_state_diagram_subcommand() {
        let cli = parse_cli_from(["cargo-statum-graph", "state-diagram", "/tmp/workspace"]);

        let Cli::StateDiagram(args) = cli else {
            panic!("expected state-diagram subcommand");
        };
        assert_eq!(args.path, PathBuf::from("/tmp/workspace"));
    }

    #[test]
    fn parse_cli_from_accepts_sequence_diagram_subcommand() {
        let cli = parse_cli_from(["cargo-statum-graph", "sequence-diagram", "/tmp/workspace"]);

        let Cli::SequenceDiagram(args) = cli else {
            panic!("expected sequence-diagram subcommand");
        };
        assert_eq!(args.path, PathBuf::from("/tmp/workspace"));
    }
}
