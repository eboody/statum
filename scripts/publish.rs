// cargo-deps: toml="0.8.8"

extern crate toml;

use std::fs;
use std::io;
use std::process::Command;
use std::thread::sleep;
use std::time::Duration;
use toml::Value;

const PUBLISH_ORDER: [&str; 5] = [
    "module_path_extractor",
    "macro_registry",
    "statum-core",
    "statum-macros",
    "statum",
];

fn run(mut cmd: Command, context: &str) -> Result<(), Box<dyn std::error::Error>> {
    let output = cmd.output()?;
    if output.status.success() {
        return Ok(());
    }

    if !output.stdout.is_empty() {
        println!("stdout:\n{}", String::from_utf8_lossy(&output.stdout));
    }
    if !output.stderr.is_empty() {
        eprintln!("stderr:\n{}", String::from_utf8_lossy(&output.stderr));
    }
    Err(format!("{context} failed").into())
}

fn ensure_clean_worktree() -> Result<(), Box<dyn std::error::Error>> {
    let output = Command::new("git")
        .args(["status", "--porcelain"])
        .output()?;
    if !output.status.success() {
        return Err("Failed to inspect git status".into());
    }
    if !output.stdout.is_empty() {
        return Err(
            "Working tree is not clean. Commit or stash changes before running publish script."
                .into(),
        );
    }
    Ok(())
}

fn crate_version(crate_name: &str) -> Result<String, Box<dyn std::error::Error>> {
    let cargo_content = fs::read_to_string(format!("{crate_name}/Cargo.toml"))?;
    let cargo_toml: Value = toml::from_str(&cargo_content)?;
    cargo_toml["package"]["version"]
        .as_str()
        .map(ToOwned::to_owned)
        .ok_or_else(|| "Version not found".into())
}

fn verify_versions_match() -> Result<String, Box<dyn std::error::Error>> {
    let first = crate_version(PUBLISH_ORDER[0])?;
    for crate_name in PUBLISH_ORDER.iter().skip(1) {
        let version = crate_version(crate_name)?;
        if version != first {
            return Err(format!(
                "Version mismatch: {} has {}, expected {}",
                crate_name, version, first
            )
            .into());
        }
    }
    Ok(first)
}

fn read_line_trimmed() -> Result<String, Box<dyn std::error::Error>> {
    let mut input = String::new();
    io::stdin().read_line(&mut input)?;
    Ok(input.trim().to_string())
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    ensure_clean_worktree()?;

    println!("Enter version increment (e.g. 0.0.1):");
    let increment = read_line_trimmed()?;
    if increment.is_empty() {
        return Err("Version increment cannot be empty".into());
    }

    println!("\nIncrementing versions...");
    run(
        {
            let mut cmd = Command::new("cargo");
            cmd.args(["script", "scripts/update_version.rs", "--", &increment]);
            cmd
        },
        "Version increment",
    )?;

    let version = verify_versions_match()?;
    println!("✓ All publishable crates are aligned at version {version}");

    println!("\nRunning pre-publish checks...");
    run(
        {
            let mut cmd = Command::new("cargo");
            cmd.args(["fmt", "--all", "--check"]);
            cmd
        },
        "cargo fmt --check",
    )?;
    run(
        {
            let mut cmd = Command::new("cargo");
            cmd.args([
                "clippy",
                "--workspace",
                "--all-targets",
                "--all-features",
                "--",
                "-D",
                "warnings",
            ]);
            cmd
        },
        "cargo clippy",
    )?;
    run(
        {
            let mut cmd = Command::new("cargo");
            cmd.args(["test", "--workspace"]);
            cmd
        },
        "cargo test",
    )?;

    println!("\nRunning publish dry-runs in dependency order...");
    for crate_name in PUBLISH_ORDER {
        println!("Dry-run publishing {crate_name}...");
        run(
            {
                let mut cmd = Command::new("cargo");
                cmd.args(["publish", "-p", crate_name, "--dry-run"]);
                cmd
            },
            &format!("cargo publish --dry-run for {crate_name}"),
        )?;
    }

    println!(
        "\nDry-runs passed for version {version}. Type 'publish' to continue with actual publish:"
    );
    let confirm = read_line_trimmed()?;
    if confirm != "publish" {
        return Err("Publish aborted by user".into());
    }

    for (idx, crate_name) in PUBLISH_ORDER.iter().enumerate() {
        println!("\nPublishing {crate_name}...");
        run(
            {
                let mut cmd = Command::new("cargo");
                cmd.args(["publish", "-p", crate_name]);
                cmd
            },
            &format!("cargo publish for {crate_name}"),
        )?;

        if idx + 1 != PUBLISH_ORDER.len() {
            println!("Waiting 30 seconds before publishing next crate...");
            sleep(Duration::from_secs(30));
        }
    }

    println!("\n✓ All crates published successfully.");
    Ok(())
}
