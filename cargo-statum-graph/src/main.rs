use std::path::PathBuf;
use std::process::ExitCode;

use clap::{Args, Parser};

use cargo_statum_graph::{run, Options};

#[derive(Debug, Parser)]
#[command(name = "cargo-statum-graph")]
#[command(about = "Generate static Statum graph bundles for existing crates")]
enum Cli {
    #[command(name = "codebase")]
    Codebase(CodebaseArgs),
}

#[derive(Debug, Args)]
struct CodebaseArgs {
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

fn main() -> ExitCode {
    match run_main() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("{error}");
            ExitCode::FAILURE
        }
    }
}

fn run_main() -> Result<(), cargo_statum_graph::Error> {
    let cli = Cli::parse();
    let written = match cli {
        Cli::Codebase(args) => run(Options {
            input_path: args.manifest_path.unwrap_or(args.path),
            package: args.package,
            out_dir: args.out_dir,
            stem: args.stem,
            patch_statum_root: args.patch_statum_root,
        })?,
    };

    for path in written {
        println!("{}", path.display());
    }

    Ok(())
}
