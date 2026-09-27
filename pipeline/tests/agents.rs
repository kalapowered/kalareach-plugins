//! Every connector package has a qualification record for the build its table is pinned to.
//!
//! A connector table is qualified against one upstream build, and section 12 asks for eight cases
//! to be run against that build and recorded per operating system and architecture. The records are
//! under `fixtures/agents/<publisher>/<plugin>/<version>/<os>-<arch>.json`, in the form
//! `fixtures/agents/README.md` states. This reads each package's pinned build out of its own table
//! and requires the build list to pin that build and the record for it to be in good form, bound to
//! this release of the package by its manifest digest and naming the build's SHA-256: the checks
//! `kalareach_catalogue::records` makes, which the pipeline also makes before it names a build. A
//! record no package pins is a stale record, and fails the same way.
//!
//! What this cannot say is whether a case qualifies: a record that names a part as not run is still
//! a record. Acceptance is read from the outcomes, part by part, and `kalareach_catalogue::builds`
//! is where the index reads them.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use kalareach_catalogue::records::{self, RECORDS};
use kalareach_catalogue::{builds, packages, repository_root};
use serde_json::Value;

fn root() -> PathBuf {
    repository_root(Path::new(env!("CARGO_MANIFEST_DIR"))).expect("the repository root is above us")
}

#[test]
fn every_connector_package_has_a_qualification_record_for_its_pinned_build() {
    let root = root();
    let loaded = packages::load(&root).expect("the repository loads");
    let list = builds::read_list(&root)
        .expect("the build list reads")
        .expect("the repository has a build list");
    let mut problems = Vec::new();
    let mut pinned = BTreeSet::new();
    for package in &loaded.repository.packages {
        let Some(connector) = &package.package.connector else {
            continue;
        };
        let manifest = &package.package.manifest;
        let tested = connector.protocol.tested_version.to_string();
        let relative = records::path(
            manifest.publisher_id.as_str(),
            manifest.plugin_name.as_str(),
            &tested,
            &list.platform,
        );
        pinned.insert(relative.clone());
        let Some(build) = list.builds.iter().find(|build| {
            build.package == manifest.plugin_id().as_str() && build.version.to_string() == tested
        }) else {
            problems.push(format!(
                "{}: the build list pins no build {tested}, the version its table is pinned to",
                manifest.plugin_id()
            ));
            continue;
        };
        let bytes = match std::fs::read(root.join(&relative)) {
            Ok(bytes) => bytes,
            Err(error) => {
                problems.push(format!(
                    "{}: no record for its pinned build {tested} at {} ({error})",
                    manifest.plugin_id(),
                    relative.display()
                ));
                continue;
            }
        };
        match serde_json::from_slice::<Value>(&bytes) {
            Ok(record) => {
                let found = records::form_problems(&record, &list.platform)
                    .into_iter()
                    .chain(records::release_problems(&record, package))
                    .chain(records::build_problems(
                        &record,
                        build.application.as_str(),
                        &tested,
                        &build.sha256.to_string(),
                    ));
                problems.extend(found.map(|problem| format!("{}: {problem}", relative.display())));
            }
            Err(error) => problems.push(format!("{}: not JSON: {error}", relative.display())),
        }
    }
    for relative in record_files(&root) {
        if !pinned.contains(&relative) {
            problems.push(format!(
                "{} is a record for a build no connector package pins",
                relative.display()
            ));
        }
    }
    assert!(
        problems.is_empty(),
        "the qualification records do not match the packages:\n{}",
        problems.join("\n")
    );
}

/// Every record file under the records directory, relative to the repository root: the files four
/// directories down, which is where a record for one publisher, plugin, version and platform is.
fn record_files(root: &Path) -> Vec<PathBuf> {
    let mut found = Vec::new();
    let mut directories = vec![(root.join(RECORDS), 0_usize)];
    while let Some((directory, depth)) = directories.pop() {
        let Ok(entries) = std::fs::read_dir(&directory) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                directories.push((path, depth + 1));
            } else if depth == 3
                && let Ok(relative) = path.strip_prefix(root)
            {
                found.push(relative.to_path_buf());
            }
        }
    }
    found.sort();
    found
}
