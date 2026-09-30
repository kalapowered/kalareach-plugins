//! A qualification record names what was run and what it showed, and nothing that could change what a
//! host allows.
//!
//! | Row | What proves it |
//! | --- | --- |
//! | KR-REQ-11.18 | a record with a `capabilities` or a `grant` member is refused by the pipeline, and the members an index carries for a qualified build are the SDK's and no others |
//!
//! The control is the same record without the added member, which loads.

use std::path::{Path, PathBuf};

use kalareach_catalogue::{builds, index, packages, records, repository_root};
use kr_plugin_sdk::catalogue::{IndexEntry, QualifiedBuild};
use kr_plugin_sdk::digest::PayloadDigest;
use kr_plugin_sdk::ids::RepositoryGeneration;
use kr_plugin_sdk::scalars::TimestampMs;
use kr_plugin_sdk::text::Label;
use kr_plugin_sdk::version::PackageVersion;
use serde_json::{Value, json};

fn root() -> PathBuf {
    repository_root(Path::new(env!("CARGO_MANIFEST_DIR"))).expect("the repository root is above us")
}

/// Copies one directory tree to another.
fn copy_tree(from: &Path, to: &Path) {
    std::fs::create_dir_all(to).expect("the destination directory");
    for entry in std::fs::read_dir(from).expect("the source directory reads") {
        let entry = entry.expect("a readable entry");
        let target = to.join(entry.file_name());
        if entry.path().is_dir() {
            copy_tree(&entry.path(), &target);
        } else {
            std::fs::copy(entry.path(), target).expect("the file copies");
        }
    }
}

/// A repository in `directory` holding the publisher record, a copy of the Claude Code package, a
/// build list pinning its build, and `record` as the record for that build.
fn repository_with_a_record(directory: &Path, record: &Value) {
    std::fs::create_dir_all(directory.join("publishers")).expect("publishers directory");
    std::fs::copy(
        root().join("publishers/kalareach.json"),
        directory.join("publishers/kalareach.json"),
    )
    .expect("the publisher record copies");
    copy_tree(
        &root().join("plugins/kalareach/claude-code"),
        &directory.join("plugins/kalareach/claude-code"),
    );
    let list: Value = serde_json::from_slice(
        &std::fs::read(root().join(builds::LIST)).expect("the build list reads"),
    )
    .expect("the build list is JSON");
    let pin = list["builds"]
        .as_array()
        .expect("builds")
        .iter()
        .find(|pin| pin["package"] == "kalareach/claude-code")
        .expect("the list pins a Claude Code build")
        .clone();
    let write = |path: PathBuf, value: &Value| {
        std::fs::create_dir_all(path.parent().expect("a parent directory"))
            .expect("the directory creates");
        std::fs::write(path, serde_json::to_string_pretty(value).expect("renders"))
            .expect("the document writes");
    };
    write(
        directory.join(builds::LIST),
        &json!({ "list_version": 1, "platform": "macos-aarch64", "builds": [pin] }),
    );
    write(
        directory.join("fixtures/agents/kalareach/claude-code/2.1.278/macos-aarch64.json"),
        record,
    );
}

/// A qualification record names what was run and what it showed. One that adds a `capabilities` or a
/// `grant` member at its top level is refused by the pipeline: the record's own form check names
/// the member, `packages::load` refuses the repository, and no build of it reaches an index. The
/// same record without the member loads. Members inside the record's own objects are not checked
/// here: nothing reads them for what they say beyond the form's fields, and what an index carries
/// for a qualified build is exactly the SDK's members, which the last part of this case holds.
///
/// KR-REQ-11.18.
#[test]
fn kr_req_11_18_a_record_with_an_added_capabilities_or_grant_member_is_refused() {
    let committed: Value = serde_json::from_slice(
        &std::fs::read(
            root().join("fixtures/agents/kalareach/claude-code/2.1.278/macos-aarch64.json"),
        )
        .expect("the record reads"),
    )
    .expect("the record is JSON");
    let platform = records::Platform::parse("macos-aarch64").expect("a platform");
    assert_eq!(
        records::form_problems(&committed, &platform),
        Vec::<String>::new(),
        "the committed record is in good form"
    );

    let temporary = tempfile::tempdir().expect("a temporary directory");
    let control = temporary.path().join("control");
    repository_with_a_record(&control, &committed);
    packages::load(&control).expect("a record without the member loads");

    for member in ["capabilities", "grant"] {
        let mut altered = committed.clone();
        altered[member] = json!({ "capabilities": ["process.spawn"] });
        let problems = records::form_problems(&altered, &platform);
        assert!(
            problems
                .iter()
                .any(|problem| problem.starts_with(&format!("{member} is not a member"))),
            "{member}: {problems:?}"
        );
        let directory = temporary.path().join(member);
        repository_with_a_record(&directory, &altered);
        let refusal = packages::load(&directory)
            .expect_err("the repository is refused")
            .to_string();
        assert!(
            refusal.contains(&format!("{member} is not a member of a record")),
            "{member}: {refusal}"
        );
    }

    // The members an index carries for a build are the SDK's: a qualified build written out has
    // exactly these, and a document that adds a member is not an entry.
    let loaded = packages::load(&root()).expect("the repository loads");
    let entry = index::build(
        &loaded.repository,
        RepositoryGeneration::new(1),
        TimestampMs::new(1_760_000_000_000),
    )
    .entries[0]
        .clone();
    let mut named = entry.clone();
    let listed = named.platforms[0].clone();
    named.builds = vec![QualifiedBuild {
        application: Label::new("example-agent").expect("a label"),
        distribution: Label::new("npm @kalareach/example-agent").expect("a label"),
        version: PackageVersion::parse("1.4.0").expect("a version"),
        os: listed.os,
        architecture: listed.architectures[0],
        executable_digest: PayloadDigest::of(b"1.4.0"),
    }];
    let document = serde_json::to_value(&named).expect("serialisable");
    let mut members: Vec<&str> = document["builds"][0]
        .as_object()
        .expect("an object")
        .keys()
        .map(String::as_str)
        .collect();
    members.sort_unstable();
    assert_eq!(
        members,
        [
            "application",
            "architecture",
            "distribution",
            "executable_digest",
            "os",
            "version"
        ]
    );
    for member in ["capabilities", "grant"] {
        let mut altered = document.clone();
        altered["builds"][0][member] = json!({ "capabilities": ["process.spawn"] });
        let refusal =
            serde_json::from_value::<IndexEntry>(altered).expect_err("a member no build carries");
        assert!(
            refusal.to_string().contains("unknown field"),
            "{member}: {refusal}"
        );
    }
}
