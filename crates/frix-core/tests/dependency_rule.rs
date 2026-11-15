//! Dependency rule between crates (docs/PROJECT.md §6.1):
//! `frix-mcp` and `frix-web` depend on `frix-core`; `frix-core` depends on
//! neither; `frix-mcp` and `frix-web` do not know each other.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::path::PathBuf;

/// Names of the workspace crates that appear as a dependency in the manifest
/// of `crate_dir`. Only considers lines of `*dependencies*` tables.
fn internal_dependencies(crate_dir: &str) -> Vec<String> {
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join(crate_dir)
        .join("Cargo.toml");
    let text = std::fs::read_to_string(&manifest)
        .unwrap_or_else(|e| panic!("manifest {} not readable: {e}", manifest.display()));

    let mut in_dependencies = false;
    let mut found = Vec::new();
    for line in text.lines().map(str::trim) {
        if line.starts_with('[') {
            in_dependencies = line.contains("dependencies");
            continue;
        }
        if !in_dependencies {
            continue;
        }
        let name = line.split(['=', '.']).next().unwrap_or("").trim();
        if ["frix-core", "frix-mcp", "frix-web"].contains(&name) {
            found.push(name.to_owned());
        }
    }
    found
}

#[test]
fn frix_core_does_not_depend_on_the_binaries() {
    assert_eq!(internal_dependencies("frix-core"), Vec::<String>::new());
}

#[test]
fn frix_mcp_depends_only_on_frix_core() {
    assert_eq!(internal_dependencies("frix-mcp"), vec!["frix-core"]);
}

#[test]
fn frix_web_depends_only_on_frix_core() {
    assert_eq!(internal_dependencies("frix-web"), vec!["frix-core"]);
}
