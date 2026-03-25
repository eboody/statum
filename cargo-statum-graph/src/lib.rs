use std::env;
use std::fmt;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitStatus};

use cargo_metadata::{Metadata, MetadataCommand, Package, PackageId};
use tempfile::TempDir;

const GRAPH_EXTENSIONS: [&str; 4] = ["mmd", "dot", "puml", "json"];

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Options {
    pub input_path: PathBuf,
    pub package: Option<String>,
    pub out_dir: Option<PathBuf>,
    pub stem: String,
    pub patch_statum_root: Option<PathBuf>,
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
    Io {
        action: &'static str,
        path: PathBuf,
        source: io::Error,
    },
    RunnerFailed {
        manifest_path: PathBuf,
        status: ExitStatus,
        details: Option<String>,
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
                manifest_path,
                status,
                details,
            } => match details {
                Some(details) => write!(
                    formatter,
                    "codebase export for `{}` failed:\n{details}",
                    manifest_path.display()
                ),
                None => write!(
                    formatter,
                    "codebase export for `{}` failed with status {status}",
                    manifest_path.display()
                ),
            },
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
            | Self::RunnerFailed { .. } => None,
        }
    }
}

pub fn run(options: Options) -> Result<Vec<PathBuf>, Error> {
    let input_path = absolutize(&options.input_path).map_err(Error::CurrentDir)?;
    let input = resolve_input(&input_path);
    let metadata = load_metadata(&input.manifest_path)?;
    let selections = select_packages(&metadata, &input, options.package.as_deref())?;
    let out_dir = resolve_out_dir(&input, options.out_dir.as_deref())?;
    let patch_root = match options.patch_statum_root {
        Some(path) => Some(absolutize(&path).map_err(Error::CurrentDir)?),
        None => detect_patch_root(),
    };

    let temp_dir = TempDir::new().map_err(|source| Error::Io {
        action: "create temporary runner directory",
        path: env::temp_dir(),
        source,
    })?;
    write_runner_project(
        temp_dir.path(),
        &selections,
        &out_dir,
        &options.stem,
        patch_root.as_deref(),
    )?;
    run_runner(temp_dir.path().join("Cargo.toml"), &input.manifest_path)?;

    Ok(bundle_paths(&out_dir, &options.stem))
}

fn load_metadata(manifest_path: &Path) -> Result<Metadata, Error> {
    MetadataCommand::new()
        .manifest_path(manifest_path)
        .no_deps()
        .exec()
        .map_err(Error::Metadata)
}

fn select_packages<'a>(
    metadata: &'a Metadata,
    input: &ResolvedInput,
    requested: Option<&str>,
) -> Result<Vec<SelectedPackage<'a>>, Error> {
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
    metadata.workspace_root.as_std_path().join("Cargo.toml")
}

fn workspace_packages<'a>(metadata: &'a Metadata, ids: &[PackageId]) -> Vec<&'a Package> {
    ids.iter()
        .filter_map(|id| metadata.packages.iter().find(|package| package.id == *id))
        .collect()
}

fn has_library_target(package: &Package) -> bool {
    package.targets.iter().any(|target| {
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

fn write_runner_project(
    runner_dir: &Path,
    selections: &[SelectedPackage<'_>],
    out_dir: &Path,
    stem: &str,
    patch_root: Option<&Path>,
) -> Result<(), Error> {
    let src_dir = runner_dir.join("src");
    fs::create_dir_all(&src_dir).map_err(|source| Error::Io {
        action: "create runner source directory",
        path: src_dir.clone(),
        source,
    })?;

    let manifest_path = runner_dir.join("Cargo.toml");
    let manifest = build_runner_manifest(selections, patch_root);
    fs::write(&manifest_path, manifest).map_err(|source| Error::Io {
        action: "write generated runner manifest",
        path: manifest_path.clone(),
        source,
    })?;

    let main_path = src_dir.join("main.rs");
    let main = build_runner_main(selections, out_dir, stem);
    fs::write(&main_path, main).map_err(|source| Error::Io {
        action: "write generated runner source",
        path: main_path.clone(),
        source,
    })?;

    Ok(())
}

fn build_runner_manifest(selections: &[SelectedPackage<'_>], patch_root: Option<&Path>) -> String {
    let mut manifest = String::from(
        "[package]\nname = \"statum-graph-runner\"\nversion = \"0.0.0\"\nedition = \"2021\"\npublish = false\n\n[dependencies]\n",
    );
    for (index, selection) in selections.iter().enumerate() {
        manifest.push_str(&format!(
            "{} = {{ package = {}, path = {} }}\n",
            selection.dependency_alias(index),
            toml_str(selection.package.name.as_ref()),
            toml_path(
                selection
                    .package
                    .manifest_path
                    .as_std_path()
                    .parent()
                    .expect("package manifest should have a parent")
            )
        ));
    }

    match patch_root {
        Some(root) => {
            manifest.push_str(&format!(
                "statum-graph = {{ path = {} }}\n",
                toml_path(root.join("statum-graph"))
            ));
            push_patch_tables(&mut manifest, root);
        }
        None => {
            manifest.push_str(&format!(
                "statum-graph = {{ version = {} }}\n",
                toml_str(&format!("={}", env!("CARGO_PKG_VERSION")))
            ));
        }
    }

    manifest
}

fn push_patch_tables(manifest: &mut String, root: &Path) {
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
                toml_path(root.join(package))
            ));
        }
    }
}

fn build_runner_main(selections: &[SelectedPackage<'_>], out_dir: &Path, stem: &str) -> String {
    let mut source = String::from("#[allow(unused_imports)]\n");
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
    source.push_str("    let doc = statum_graph::CodebaseDoc::linked()?;\n");
    source.push_str("    statum_graph::codebase::render::write_all_to_dir(\n");
    source.push_str("        &doc,\n");
    source.push_str(&format!(
        "        {},\n",
        rust_str(&out_dir.to_string_lossy())
    ));
    source.push_str(&format!("        {},\n", rust_str(stem)));
    source.push_str("    )?;\n");
    source.push_str("    Ok(())\n");
    source.push_str("}\n");
    source
}

fn run_runner(runner_manifest_path: PathBuf, target_manifest_path: &Path) -> Result<(), Error> {
    let output = Command::new("cargo")
        .arg("run")
        .arg("--quiet")
        .arg("--manifest-path")
        .arg(&runner_manifest_path)
        .output()
        .map_err(|source| Error::Io {
            action: "run generated cargo runner",
            path: runner_manifest_path.clone(),
            source,
        })?;

    if output.status.success() {
        Ok(())
    } else {
        Err(Error::RunnerFailed {
            manifest_path: target_manifest_path.to_path_buf(),
            status: output.status,
            details: normalize_runner_failure_details(&output.stderr, &output.stdout),
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

fn bundle_paths(out_dir: &Path, stem: &str) -> Vec<PathBuf> {
    GRAPH_EXTENSIONS
        .into_iter()
        .map(|extension| out_dir.join(format!("{stem}.{extension}")))
        .collect()
}

fn absolutize(path: &Path) -> io::Result<PathBuf> {
    if path.is_absolute() {
        Ok(path.to_path_buf())
    } else {
        Ok(env::current_dir()?.join(path))
    }
}

fn toml_path(value: impl AsRef<Path>) -> String {
    let value = value.as_ref().to_string_lossy();
    format!("\"{}\"", value.replace('\\', "\\\\").replace('"', "\\\""))
}

fn rust_str(value: &str) -> String {
    format!("\"{}\"", value.replace('\\', "\\\\").replace('"', "\\\""))
}

fn toml_str(value: &str) -> String {
    format!("\"{}\"", value.replace('\\', "\\\\").replace('"', "\\\""))
}

struct SelectedPackage<'a> {
    package: &'a Package,
}

impl<'a> SelectedPackage<'a> {
    fn new(package: &'a Package, manifest_path: &Path) -> Result<Self, Error> {
        if has_library_target(package) {
            Ok(Self { package })
        } else {
            Err(Error::PackageHasNoLibrary {
                manifest_path: manifest_path.to_path_buf(),
                package: package.name.to_string(),
            })
        }
    }

    fn dependency_alias(&self, index: usize) -> String {
        format!("graph_target_{index}")
    }
}

struct ResolvedInput {
    manifest_path: PathBuf,
    default_output_dir: PathBuf,
}
