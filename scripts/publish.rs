// cargo-deps: toml="0.8.8"

extern crate toml;

use std::fs;
use std::io::Write;
use std::process::{Command, Stdio};
use std::thread::sleep;
use std::time::Duration;
use toml::Value;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let crates = ["statum-core", "statum-macros", "statum"];

    // First ask for version increment
    println!("Enter version increment (e.g. 0.0.1):");
    let mut increment = String::new();
    std::io::stdin().read_line(&mut increment)?;
    let increment = increment.trim();

    // Run the version increment script
    println!("\nIncrementing versions...");
    let output = Command::new("cargo")
        .args(["script", "scripts/update_version.rs", "--", increment])
        .output()?;

    if !output.status.success() {
        println!("stdout:\n{}", String::from_utf8_lossy(&output.stdout));
        eprintln!("stderr:\n{}", String::from_utf8_lossy(&output.stderr));
        return Err("Version increment failed".into());
    }

    println!("✓ Version increment successful");

    // Git add
    println!("\nAdding changes to git...");
    let output = Command::new("git").args(["add", "*"]).output()?;

    if !output.status.success() {
        return Err("Git add failed".into());
    }

    // First get commit type using lumen list and fzf
    println!("Getting commit type...");
    let mut lumen = Command::new("lumen")
        .arg("list")
        .stdout(Stdio::piped())
        .spawn()?;

    let mut fzf = Command::new("fzf")
        .arg("-n1")
        .arg("--select-1")
        .stdin(lumen.stdout.take().ok_or("Failed to get lumen stdout")?)
        .stdout(Stdio::piped())
        .spawn()?;

    let output = fzf.wait_with_output()?;
    lumen.wait()?;

    if !output.status.success() {
        return Err("Failed to get commit type".into());
    }

    let commit_type = String::from_utf8(output.stdout)?.trim().to_string();
    println!("Selected commit type: {}", commit_type);

    // Now use that type to get the commit message
    let draft_output = Command::new("lumen")
        .arg("draft")
        .arg(&commit_type)
        .output()?;

    if !draft_output.status.success() {
        eprintln!("stderr:\n{}", String::from_utf8_lossy(&draft_output.stderr));
        return Err("Lumen draft failed".into());
    }

    let commit_msg = String::from_utf8(draft_output.stdout)?;
    println!("\nCommit message:\n{}", commit_msg);

    // Create git commit with the message
    let mut commit = Command::new("git")
        .args(["commit", "-F", "-"])
        .stdin(Stdio::piped())
        .spawn()?;

    if let Some(mut stdin) = commit.stdin.take() {
        stdin.write_all(commit_msg.as_bytes())?;
    }

    let status = commit.wait()?;
    if !status.success() {
        return Err("Git commit failed".into());
    }

    println!("✓ Changes committed successfully");

    // Get the new version
    let cargo_content = fs::read_to_string(format!("{}/Cargo.toml", crates[0]))?;
    let cargo_toml: Value = toml::from_str(&cargo_content)?;
    let new_version = cargo_toml["package"]["version"]
        .as_str()
        .ok_or("Version not found")?;

    // Check if user wants to continue with validation and publishing
    println!(
        "\nVersions updated to {}. Continue with validation and publishing?",
        new_version
    );
    println!("Press Enter to continue or Ctrl+C to abort...");
    let mut input = String::new();
    std::io::stdin().read_line(&mut input)?;

    // First verify all versions match
    let mut versions = Vec::new();
    for crate_name in crates {
        let cargo_content = fs::read_to_string(format!("{}/Cargo.toml", crate_name))?;
        let cargo_toml: Value = toml::from_str(&cargo_content)?;
        let version = cargo_toml["package"]["version"]
            .as_str()
            .ok_or("Version not found")?;
        versions.push((crate_name, version.to_string()));
    }

    // Check all versions match
    let first_version = &versions[0].1;
    for (crate_name, version) in &versions {
        if version != first_version {
            return Err(format!(
                "Version mismatch! {} has version {} but {} has version {}",
                crates[0], first_version, crate_name, version
            )
            .into());
        }
    }
    println!("✓ All crate versions match: {}", first_version);

    // Verify all crates build
    println!("\nVerifying builds...");
    for crate_name in crates {
        println!("\nBuilding {}...", crate_name);
        let output = Command::new("cargo")
            .current_dir(crate_name)
            .args(["build", "--all-features"])
            .output()?;

        if !output.status.success() {
            println!("stdout:\n{}", String::from_utf8_lossy(&output.stdout));
            eprintln!("stderr:\n{}", String::from_utf8_lossy(&output.stderr));
            return Err(format!("Failed to build {}", crate_name).into());
        }
        println!("✓ {} built successfully", crate_name);
    }

    // Verify tests pass
    println!("\nRunning tests...");
    for crate_name in crates {
        println!("\nTesting {}...", crate_name);
        let output = Command::new("cargo")
            .current_dir(crate_name)
            .args(["test", "--all-features"])
            .output()?;

        if !output.status.success() {
            println!("stdout:\n{}", String::from_utf8_lossy(&output.stdout));
            eprintln!("stderr:\n{}", String::from_utf8_lossy(&output.stderr));
            return Err(format!("Tests failed for {}", crate_name).into());
        }
        println!("✓ {} tests passed", crate_name);
    }

    // Run dry-run publishes first
    println!("\nPerforming dry-run publishes...");
    for crate_name in crates {
        println!("\nDry-run publishing {}...", crate_name);
        let output = Command::new("cargo")
            .current_dir(crate_name)
            .args(["publish", "--dry-run"])
            .output()?;

        if !output.status.success() {
            println!("stdout:\n{}", String::from_utf8_lossy(&output.stdout));
            eprintln!("stderr:\n{}", String::from_utf8_lossy(&output.stderr));
            return Err(format!("Dry-run publish failed for {}", crate_name).into());
        }
        println!("✓ {} dry-run publish successful", crate_name);
    }

    // Check if user wants to continue with actual publish
    println!(
        "\nAll checks and dry-runs passed! Ready to publish version {}.",
        first_version
    );
    println!("Press Enter to continue with actual publish or Ctrl+C to abort...");
    let mut input = String::new();
    std::io::stdin().read_line(&mut input)?;

    // Publish crates
    for crate_name in crates {
        println!("\nPublishing {}", crate_name);

        let output = Command::new("cargo")
            .current_dir(crate_name)
            .args(["publish", "--no-verify"]) // Skip verification since we already did it
            .output()?;

        if !output.stdout.is_empty() {
            println!("stdout:\n{}", String::from_utf8_lossy(&output.stdout));
        }
        if !output.stderr.is_empty() {
            eprintln!("stderr:\n{}", String::from_utf8_lossy(&output.stderr));
        }

        if !output.status.success() {
            return Err(format!("Failed to publish {}", crate_name).into());
        }

        println!("✓ Successfully published {}", crate_name);

        // Sleep between publishes
        if crate_name != crates[crates.len() - 1] {
            println!("Waiting 30 seconds before publishing next crate...");
            sleep(Duration::from_secs(30));
        }
    }

    println!("\n✓ All crates published successfully!");
    Ok(())
}
