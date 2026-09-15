//! The synthetic catalogue benchmark for 10,000 packages.
//!
//! Section 21 requires a catalogue of 10,000 small definitions that a host can sync and search
//! offline. This measures the two numbers that decide whether that is affordable: how long the
//! index takes to build and render, and how large it is.

use std::path::{Path, PathBuf};

use kalareach_catalogue::{bench, packages, repository_root};

fn root() -> PathBuf {
    repository_root(Path::new(env!("CARGO_MANIFEST_DIR"))).expect("the repository root is above us")
}

#[test]
fn a_ten_thousand_entry_catalogue_builds_and_searches() {
    let loaded = packages::load(&root()).expect("the repository loads");
    let template = loaded
        .repository
        .packages
        .iter()
        .find(|package| package.package.manifest.plugin_name.as_str() == "example-declarative")
        .expect("the example package is present");

    let measurement = bench::measure(&template.package.manifest, 10_000);
    println!("{}", measurement.report());

    assert_eq!(measurement.entries, 10_000);
    // The default repository metadata budget is 64 MiB for 100,000 entries. A catalogue a tenth
    // that size has to fit inside it with room to spare, or offline search is not affordable.
    assert!(
        measurement.bytes < 64 * 1024 * 1024,
        "the index is {} bytes, over the default 64 MiB metadata budget",
        measurement.bytes
    );
    // Offline search covers the whole index and must not feel like work.
    assert!(
        measurement.lookup.as_millis() < 1_000,
        "one offline lookup took {:?}",
        measurement.lookup
    );
}
