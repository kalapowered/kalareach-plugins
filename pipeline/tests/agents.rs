//! Every connector package has a qualification record for the build its table is pinned to.
//!
//! A connector table is qualified against one upstream build, and section 12 asks for eight cases
//! to be run against that build and recorded per operating system and architecture. The records are
//! under `fixtures/agents/<publisher>/<plugin>/<version>/<os>-<arch>.json`, in the form
//! `fixtures/agents/README.md` states. `kalareach_catalogue::builds::check_records` holds each
//! package's record to it: in good form, bound to this release of the package by its manifest
//! digest, and run against the build the table is pinned to, with the SHA-256 the build list pins
//! where it pins that build. A record no table and no pinned build names is a stale record, and
//! fails the same way.
//!
//! What this cannot say is whether a case qualifies: a record that names a part as not run is still
//! a record. Acceptance is read from the outcomes, part by part, and `kalareach_catalogue::builds`
//! is where the index reads them.

use std::path::{Path, PathBuf};

use kalareach_catalogue::{builds, packages, repository_root};

fn root() -> PathBuf {
    repository_root(Path::new(env!("CARGO_MANIFEST_DIR"))).expect("the repository root is above us")
}

#[test]
fn every_connector_package_has_a_qualification_record_for_its_pinned_build() {
    let root = root();
    let loaded = packages::load(&root).expect("the repository loads");
    let problems =
        builds::check_records(&root, &loaded.repository.packages).expect("the records read");
    assert!(
        problems.is_empty(),
        "the qualification records do not match the packages:\n{}",
        problems.join("\n")
    );
}
