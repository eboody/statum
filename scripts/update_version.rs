// cargo-deps: toml="0.8.8"

use std::env;
use std::fs;
use toml::Value;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = env::args().collect();
    if args.len() != 2 {
        eprintln!("Usage: {} <version_increment> (e.g. 0.0.1)", args[0]);
        std::process::exit(1);
    }

    let increment: Vec<u32> = args[1]
        .split('.')
        .map(|s| s.parse::<u32>())
        .collect::<Result<Vec<_>, _>>()?;

    if increment.len() != 3 {
        return Err("Version increment must be in format x.y.z".into());
    }

    // Paths for all our crates
    let crate_paths = ["statum", "statum-core", "statum-macros"];

    // Read current version
    let cargo_content = fs::read_to_string("statum/Cargo.toml")?;
    let cargo_toml: Value = toml::from_str(&cargo_content)?;
    let current_version = cargo_toml["package"]["version"]
        .as_str()
        .ok_or("Version not found")?;

    // Parse and increment version
    let mut parts: Vec<u32> = current_version
        .split('.')
        .map(|s| s.parse::<u32>())
        .collect::<Result<Vec<_>, _>>()?;

    if parts.len() != 3 {
        return Err("Current version must be in format x.y.z".into());
    }

    // Add the increment to each part
    for i in 0..3 {
        parts[i] += increment[i];
    }

    let new_version = format!("{}.{}.{}", parts[0], parts[1], parts[2]);
    println!(
        "Incrementing version from {} to {}",
        current_version, new_version
    );

    // Update all crates
    for crate_path in &crate_paths {
        let cargo_path = format!("{}/Cargo.toml", crate_path);
        let content = fs::read_to_string(&cargo_path)?;
        let mut doc: Value = toml::from_str(&content)?;

        // Update the crate's own version
        if let Some(package) = doc.get_mut("package") {
            if let Some(version) = package.get_mut("version") {
                *version = Value::String(new_version.clone());
            }
        }

        // Update any dependencies on our other crates
        if let Some(deps) = doc.get_mut("dependencies") {
            for dep_name in ["statum-core", "statum-macros"] {
                if let Some(dep) = deps.get_mut(dep_name) {
                    if let Some(table) = dep.as_table_mut() {
                        if let Some(version) = table.get_mut("version") {
                            *version = Value::String(new_version.clone());
                        }
                    }
                }
            }
        }

        // Write back the updated TOML
        fs::write(&cargo_path, toml::to_string_pretty(&doc)?)?;
        println!("Updated version in {}", cargo_path);

        // Copy README.md
        let readme_dest = format!("{}/README.md", crate_path);
        fs::copy("README.md", &readme_dest)?;
        println!("Copied README.md to {}", readme_dest);
    }

    Ok(())
}
