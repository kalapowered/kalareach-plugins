//! The index names each release's qualified executable builds, and nothing else.
//!
//! A build is named for a release when the build list pins it and the release's qualification
//! record for it shows every part of section 12's eight cases passed. So that the rule is tested
//! whatever the committed records show, most of these tests build repositories of their own: a copy
//! of one package, a build list pinning its build, and a record derived from the package's real
//! record in which every part is made to pass. Such a record is a test input and nothing else; it
//! never leaves the temporary directory it is written to.

use std::path::{Path, PathBuf};

use jiff::{Span, Timestamp};
use kalareach_catalogue::keys::SigningDirectory;
use kalareach_catalogue::packages::{self, Repository};
use kalareach_catalogue::tuf::{self, Expiries, METADATA_DIR, TARGETS_DIR};
use kalareach_catalogue::{Error, builds, index, repository_root};
use kr_plugin_sdk::catalogue::{
    BuildsError, CatalogueIndex, IndexEntry, MAX_QUALIFIED_BUILDS, QualifiedBuild,
};
use kr_plugin_sdk::digest::PayloadDigest;
use kr_plugin_sdk::ids::RepositoryGeneration;
use kr_plugin_sdk::matching::{Architecture, OperatingSystem};
use kr_plugin_sdk::scalars::TimestampMs;
use kr_plugin_sdk::text::Label;
use kr_plugin_sdk::version::PackageVersion;
use serde_json::{Value, json};

fn root() -> PathBuf {
    repository_root(Path::new(env!("CARGO_MANIFEST_DIR"))).expect("the repository root is above us")
}

fn fixed_time() -> TimestampMs {
    TimestampMs::new(1_760_000_000_000)
}

/// The record of this repository's Claude Code package for the build its table is pinned to.
const CLAUDE_CODE_RECORD: &str = "fixtures/agents/kalareach/claude-code/2.1.278/macos-aarch64.json";

/// The SHA-256 of the Claude Code executable the build list pins.
const CLAUDE_CODE_SHA256: &str = "bd245662fb8a0e321b3bf133e930371d6563c387527885f30b2613aef3ba14d6";

/// The SHA-256 of the newer Claude Code executable the upgrade part moves to.
const CLAUDE_CODE_NEWER_SHA256: &str =
    "a922981f6f3b55a251ef9f9dbaa0621a5f99cbcb5ca67f8a797476ccfc83f626";

fn read_json(path: &Path) -> Value {
    serde_json::from_slice(&std::fs::read(path).expect("the document reads"))
        .expect("the document is JSON")
}

fn write_json(path: &Path, value: &Value) {
    std::fs::create_dir_all(path.parent().expect("a parent directory"))
        .expect("the directory creates");
    std::fs::write(
        path,
        serde_json::to_string_pretty(value).expect("the document renders"),
    )
    .expect("the document writes");
}

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

/// The SHA-256 of the Codex executable the build list pins.
const CODEX_SHA256: &str = "8eaf1ad12fe6bf89b1710330f58900014322c7c5af677e43be116d8ac5fc0a9e";

/// The SHA-256 of the OpenCode executable both OpenCode packages pin.
const OPENCODE_SHA256: &str = "16c960ba77421da11b53e785f359b73f328a86118b48feb4af143db5d9afb198";

/// One build list entry, written here rather than read from the repository's own list, so these
/// tests hold whatever the repository pins or withdraws.
fn pin(
    package: &str,
    application: &str,
    version: &str,
    distribution: &str,
    launch: &str,
    sha256: &str,
    newer: Option<(&str, &str)>,
) -> Value {
    json!({
        "package": package,
        "application": application,
        "version": version,
        "distribution": distribution,
        "launch": launch,
        "sha256": sha256,
        "newer": newer.map(|(version, sha256)| json!({ "version": version, "sha256": sha256 })),
    })
}

/// The Claude Code build, with the newer build its upgrade part moves to.
fn claude_code_pin() -> Value {
    pin(
        "kalareach/claude-code",
        "claude-code",
        "2.1.278",
        "npm @anthropic-ai/claude-code-darwin-arm64",
        "native",
        CLAUDE_CODE_SHA256,
        Some(("2.1.281", CLAUDE_CODE_NEWER_SHA256)),
    )
}

/// The identifier a part belongs to.
fn identifier_of(part: &str) -> &'static str {
    if part.starts_with("14.03") {
        "KR-REQ-14.03"
    } else {
        "KR-REQ-12.32"
    }
}

/// Makes the counts, verdicts and summary what the tests and steps now make them.
fn recount(record: &mut Value) {
    let outcomes = [
        "passed",
        "failed",
        "ignored",
        "not_run",
        "not_built",
        "known_difference",
    ];
    for identifier in ["KR-REQ-12.32", "KR-REQ-14.03"] {
        let tests = record["identifiers"][identifier]["tests"]
            .as_array()
            .expect("the identifier lists its tests")
            .clone();
        let count = |outcome: &str| {
            tests
                .iter()
                .filter(|test| test["outcome"] == outcome)
                .count()
        };
        for outcome in outcomes {
            record["identifiers"][identifier]["counts"][outcome] = json!(count(outcome));
        }
        let verdict = if count("failed") > 0 {
            "failed"
        } else if count("passed") > 0 {
            "passed"
        } else {
            "not_run"
        };
        record["identifiers"][identifier]["verdict"] = json!(verdict);
    }
    let verdicts = |verdict: &str| {
        ["KR-REQ-12.32", "KR-REQ-14.03"]
            .iter()
            .filter(|identifier| record["identifiers"][**identifier]["verdict"] == verdict)
            .count()
    };
    let (passed, failed, not_run) = (verdicts("passed"), verdicts("failed"), verdicts("not_run"));
    record["summary"]["passed"] = json!(passed);
    record["summary"]["failed"] = json!(failed);
    record["summary"]["not_run"] = json!(not_run);
    for outcome in outcomes {
        let total: u64 = ["KR-REQ-12.32", "KR-REQ-14.03"]
            .iter()
            .map(|identifier| {
                record["identifiers"][*identifier]["counts"][outcome]
                    .as_u64()
                    .unwrap_or(0)
            })
            .sum();
        record["summary"]["tests"][outcome] = json!(total);
    }
    let failed_steps: Vec<Value> = record["steps"]
        .as_array()
        .expect("the record lists its steps")
        .iter()
        .filter(|step| step["exit"] != 0)
        .map(|step| step["number"].clone())
        .collect();
    record["summary"]["failed_steps"] = Value::Array(failed_steps);
}

/// The record with every part of both identifiers passed: each part that did not run gets a step of
/// its own that exited cleanly, each part that failed has its own step exit cleanly, and the
/// counts, verdicts and summary follow.
fn every_part_passed(record: &Value) -> Value {
    let mut record = record.clone();
    let mut next = record["steps"]
        .as_array()
        .expect("the record lists its steps")
        .iter()
        .filter_map(|step| step["number"].as_u64())
        .max()
        .unwrap_or(0);
    let mut added = Vec::new();
    let mut repaired = Vec::new();
    for identifier in ["KR-REQ-12.32", "KR-REQ-14.03"] {
        let tests = record["identifiers"][identifier]["tests"]
            .as_array_mut()
            .expect("the identifier lists its tests");
        for test in tests.iter_mut() {
            if test["outcome"] == "passed" {
                continue;
            }
            if test["outcome"] == "failed" {
                test["outcome"] = json!("passed");
                for run in test["runs"].as_array_mut().into_iter().flatten() {
                    run["outcome"] = json!("passed");
                    repaired.push(run["step"].clone());
                }
                if let Some(fields) = test.as_object_mut() {
                    fields.remove("reason");
                }
                continue;
            }
            next += 1;
            let part = test["part"].as_str().expect("a part").to_owned();
            let command = format!("the part {part} run");
            test["outcome"] = json!("passed");
            test["command"] = json!(command);
            test["runs"] = json!([{ "step": next, "outcome": "passed" }]);
            if let Some(fields) = test.as_object_mut() {
                fields.remove("reason");
                fields.remove("needs");
            }
            added.push(json!({
                "number": next,
                "group": "agents",
                "what": format!("part {part}"),
                "command": command,
                "log": format!("claude-code/{part}.log"),
                "exit": 0,
                "seconds": 1,
                "needs": [],
                "error": null,
            }));
        }
    }
    let steps = record["steps"]
        .as_array_mut()
        .expect("the record lists its steps");
    for step in steps.iter_mut() {
        if repaired.contains(&step["number"]) {
            step["exit"] = json!(0);
            step["error"] = Value::Null;
        }
    }
    steps.extend(added);
    recount(&mut record);
    record
}

/// The record with one part that ran given another outcome: `not_run`, when it names a reason and
/// its step goes, or `failed`, when its step exited with a failure.
fn with_outcome(record: &Value, part: &str, outcome: &str) -> Value {
    let mut record = record.clone();
    let tests = record["identifiers"][identifier_of(part)]["tests"]
        .as_array_mut()
        .expect("the identifier lists its tests");
    let test = tests
        .iter_mut()
        .find(|test| test["part"] == part)
        .unwrap_or_else(|| panic!("the record names part {part}"));
    let step = test["runs"][0]["step"].clone();
    match outcome {
        "not_run" => {
            test["outcome"] = json!("not_run");
            test["reason"] = json!("needs a vendor account");
            if let Some(fields) = test.as_object_mut() {
                fields.remove("command");
                fields.remove("runs");
            }
            record["steps"]
                .as_array_mut()
                .expect("the record lists its steps")
                .retain(|listed| listed["number"] != step);
        }
        "failed" => {
            test["outcome"] = json!("failed");
            test["runs"][0]["outcome"] = json!("failed");
            let steps = record["steps"]
                .as_array_mut()
                .expect("the record lists its steps");
            let listed = steps
                .iter_mut()
                .find(|listed| listed["number"] == step)
                .expect("the part's step");
            listed["exit"] = json!(101);
            listed["error"] = json!("the part's check failed");
        }
        _ => panic!("{outcome} is not an outcome this sets"),
    }
    recount(&mut record);
    record
}

/// The record of the build the list pins for Claude Code, with every part passed.
fn qualified_record() -> Value {
    every_part_passed(&read_json(&root().join(CLAUDE_CODE_RECORD)))
}

/// A repository in `directory` holding the publisher record, a copy of the Claude Code package, a
/// build list pinning its build and `record` as the record for that build.
fn claude_code_repository(directory: &Path, record: &Value) {
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
    write_list(directory, &[claude_code_pin()]);
    write_json(&directory.join(CLAUDE_CODE_RECORD), record);
}

/// Replaces the repository's build list with one pinning `builds` for macOS on Apple silicon.
fn write_list(directory: &Path, builds: &[Value]) {
    write_json(
        &directory.join(builds::LIST),
        &json!({
            "list_version": 1,
            "platform": "macos-aarch64",
            "builds": builds,
        }),
    );
}

/// Adds a copy of the example package, which has no connector table, to the repository in
/// `directory`, and returns the digest of its manifest.
fn add_example_package(directory: &Path) -> String {
    let destination = directory.join("plugins/kalareach/example-declarative");
    copy_tree(
        &root().join("plugins/kalareach/example-declarative"),
        &destination,
    );
    PayloadDigest::of(&std::fs::read(destination.join("plugin.json")).expect("the manifest reads"))
        .to_string()
}

/// A pin for a build of the example package's application.
fn example_pin() -> Value {
    pin(
        "kalareach/example-declarative",
        "example-agent",
        "1.0.0",
        "npm @kalareach/example-agent",
        "native",
        &"a".repeat(64),
        None,
    )
}

/// The build the list pins for Claude Code, as an index entry names it.
fn claude_code_build() -> QualifiedBuild {
    QualifiedBuild {
        application: Label::new("claude-code").expect("a label"),
        distribution: Label::new("npm @anthropic-ai/claude-code-darwin-arm64").expect("a label"),
        version: PackageVersion::parse("2.1.278").expect("a version"),
        os: OperatingSystem::MacOs,
        architecture: Architecture::Aarch64,
        executable_digest: PayloadDigest::parse(CLAUDE_CODE_SHA256).expect("a digest"),
    }
}

/// The index a repository builds.
fn index_of(repository: &Repository) -> CatalogueIndex {
    index::build(repository, RepositoryGeneration::new(1), fixed_time())
}

/// The index entry for Claude Code.
fn claude_code_entry(index: &mut CatalogueIndex) -> &mut IndexEntry {
    index
        .entries
        .iter_mut()
        .find(|entry| entry.plugin_id.as_str() == "kalareach/claude-code")
        .expect("the index carries Claude Code")
}

/// The builds the index entry for Claude Code names when the repository in `directory` is built.
fn claude_code_entry_builds(directory: &Path) -> Vec<QualifiedBuild> {
    let loaded = packages::load(directory).expect("the repository loads");
    claude_code_entry(&mut index_of(&loaded.repository))
        .builds
        .clone()
}

/// Why the build pinned for Claude Code is left out of the repository in `directory`, which must
/// name no build at all.
fn left_out_because(directory: &Path) -> Vec<String> {
    let loaded = packages::load(directory).expect("the repository loads");
    assert_eq!(loaded.repository.builds.named().count(), 0);
    let mut index = index_of(&loaded.repository);
    assert!(claude_code_entry(&mut index).builds.is_empty());
    let omitted = loaded.repository.builds.omitted();
    assert_eq!(omitted.len(), 1, "{omitted:?}");
    omitted[0].reasons.clone()
}

/// What refuses to load the repository in `directory`.
fn refused(directory: &Path) -> String {
    packages::load(directory)
        .expect_err("the repository is refused")
        .to_string()
}

/// A release whose record shows every part of section 12's cases passed names its pinned build:
/// the application and distribution the build list gives, the version, the platform the record's
/// file name gives, and the SHA-256 of the executable. KR-REQ-18.06: the qualified builds an entry
/// names are the compatibility data a host admits an adapter's versions from.
#[test]
fn an_entry_names_the_build_its_record_qualifies_with_its_digest_and_version() {
    let temporary = tempfile::tempdir().expect("a temporary directory");
    let repository = temporary.path().join("repository");
    claude_code_repository(&repository, &qualified_record());

    assert_eq!(
        claude_code_entry_builds(&repository),
        vec![claude_code_build()]
    );
}

/// The records as they stand name no build: each pinned build is left out for the parts of section
/// 12's cases that did not pass, and a script or a wheel for naming no executable at all. So the
/// index this repository builds is the one it built before an entry could name a build. The check
/// goes through the builds the list pins, so a pin removed to withdraw a build does not break it.
#[test]
fn no_build_is_named_while_a_section_12_part_has_not_passed() {
    let loaded = packages::load(&root()).expect("the repository loads");
    let builds = &loaded.repository.builds;
    assert_eq!(builds.named().count(), 0);
    let list = builds::read_list(&root())
        .expect("the build list reads")
        .expect("the repository has a build list");
    assert_eq!(builds.omitted().len(), list.builds.len());

    let and_upgrade = "section 12 parts 1, 2a, 2c, 3, 4, 5b, 6a, 6b, 7, 8b did not pass";
    // Claude Code's record is a run with the person's login: every part that ran passed but the
    // device's upload, and the parts that wait for the command integration did not run.
    let claude_code: &[&str] = &[
        "steps 2 did not exit cleanly",
        "parts 1 failed",
        "section 12 parts 1, 5b, 6b, 8b did not pass",
    ];
    // Codex's record is a run with the person's login: the host detected no launch of a build a
    // runtime starts, so the parts that need it failed, and the device's upload was refused.
    let codex: &[&str] = &[
        "steps 2, 4, 5, 10 did not exit cleanly",
        "parts 1, 2b, 2c, 7 failed",
        "section 12 parts 1, 2b, 2c, 5b, 6b, 7, 8b did not pass",
    ];
    // Kimi Code's record is a run with the person's login: part 1 failed at the check of the image's
    // colour in the agent's answer, before the answer to the device's upload was kept, and the agent
    // was not shown to be running its turn when part 2a's queued prompt was entered.
    let kimi_code: &[&str] = &[
        "steps 2, 3 did not exit cleanly",
        "parts 1, 2a failed",
        "section 12 parts 1, 2a, 5b, 6b, 8b did not pass",
    ];
    // OpenCode's record is a run with a provider's key in the person's shell: every part that ran
    // passed but the device's upload, and the parts that wait for the command integration did not
    // run.
    let opencode: &[&str] = &[
        "steps 2 did not exit cleanly",
        "parts 1 failed",
        "section 12 parts 1, 5b, 6b, 8b did not pass",
    ];
    let script = "the pinned file is a script, whose process is its interpreter, so it names no \
                  executable a host runs";
    let wheel = "the pinned file is a wheel, an archive, so it names no executable a host runs";
    // Gemini CLI's record is a run with a provider's key in the person's shell: a node-run build,
    // so the host detected no launch of it, and the parts that need that failed, as for Codex.
    let gemini_cli: &[&str] = &[
        script,
        "steps 2, 4, 5, 10 did not exit cleanly",
        "parts 1, 2b, 2c, 7 failed",
        "section 12 parts 1, 2b, 2c, 5b, 6b, 7, 8b did not pass",
    ];
    // Qoder CLI's record is a run with no login, which the user did not approve for it: the parts
    // that need none passed, and the parts that need one were not run.
    let qoder_cli: &[&str] = &["section 12 parts 1, 2a, 3, 4, 5b, 6b, 7, 8b did not pass"];
    let expected: [(&str, &[&str]); 8] = [
        ("kalareach/claude-code", claude_code),
        ("kalareach/codex", codex),
        ("kalareach/gemini-cli", gemini_cli),
        ("kalareach/kimi-cli", &[wheel, and_upgrade]),
        ("kalareach/kimi-code-cli", kimi_code),
        ("kalareach/opencode", opencode),
        ("kalareach/opencode-attach", &[and_upgrade]),
        ("kalareach/qoder-cli", qoder_cli),
    ];
    for pinned in &list.builds {
        let reasons = expected
            .iter()
            .find(|(plugin, _)| *plugin == pinned.package)
            .map(|(_, reasons)| *reasons)
            .unwrap_or_else(|| panic!("{} is pinned; say here why it is left out", pinned.package));
        let omitted = builds
            .omitted()
            .iter()
            .find(|omitted| omitted.plugin_id == pinned.package)
            .unwrap_or_else(|| panic!("the pinned build of {} is left out", pinned.package));
        assert_eq!(omitted.reasons, reasons, "{omitted}");
        assert_eq!(omitted.platform, "macos-aarch64");
    }

    let index = index_of(&loaded.repository);
    assert!(index.entries.iter().all(|entry| entry.builds.is_empty()));
    assert!(
        !index
            .canonical_json()
            .expect("the index renders")
            .contains("\"builds\"")
    );
}

/// Every part of section 12's cases is required: one that did not run keeps the build out, whatever
/// kept it from running.
#[test]
fn a_section_12_part_that_did_not_run_leaves_the_build_out() {
    for part in builds::REQUIRED_PARTS {
        let temporary = tempfile::tempdir().expect("a temporary directory");
        let repository = temporary.path().join("repository");
        claude_code_repository(
            &repository,
            &with_outcome(&qualified_record(), part, "not_run"),
        );
        assert_eq!(
            left_out_because(&repository),
            vec![format!("section 12 parts {part} did not pass")],
            "part {part}"
        );
    }
}

/// A part that failed keeps the build out, the typed-path rule's parts included, while a
/// typed-path part that did not run does not: that rule is not what a build claims.
#[test]
fn a_failed_part_leaves_the_build_out_and_a_typed_path_part_not_run_does_not() {
    let temporary = tempfile::tempdir().expect("a temporary directory");

    let failed = temporary.path().join("failed");
    let record = with_outcome(&qualified_record(), "5a", "failed");
    let step = record["identifiers"]["KR-REQ-12.32"]["tests"]
        .as_array()
        .expect("tests")
        .iter()
        .find(|test| test["part"] == "5a")
        .expect("part 5a")["runs"][0]["step"]
        .clone();
    claude_code_repository(&failed, &record);
    assert_eq!(
        left_out_because(&failed),
        vec![
            format!("steps {step} did not exit cleanly"),
            "parts 5a failed".to_owned(),
            "section 12 parts 5a did not pass".to_owned(),
        ]
    );

    let typed_path_failed = temporary.path().join("typed-path-failed");
    claude_code_repository(
        &typed_path_failed,
        &with_outcome(&qualified_record(), "14.03a", "failed"),
    );
    let reasons = left_out_because(&typed_path_failed);
    assert!(
        reasons.contains(&"parts 14.03a failed".to_owned()),
        "{reasons:?}"
    );

    let typed_path_not_run = temporary.path().join("typed-path-not-run");
    claude_code_repository(
        &typed_path_not_run,
        &with_outcome(&qualified_record(), "14.03b", "not_run"),
    );
    assert_eq!(
        claude_code_entry_builds(&typed_path_not_run),
        vec![claude_code_build()]
    );
}

/// A record says something about a build only as a whole run of this release and this build: one
/// for another release, another build, a run whose own-home check failed, or one with problems,
/// keeps the build out; so does a missing record.
#[test]
fn a_record_that_is_not_a_whole_run_of_this_release_and_build_leaves_the_build_out() {
    let temporary = tempfile::tempdir().expect("a temporary directory");

    let mut record = qualified_record();
    record["run"]["packages"][0]["manifest_digest"] = json!(CLAUDE_CODE_NEWER_SHA256);
    let other_release = temporary.path().join("other-release");
    claude_code_repository(&other_release, &record);
    let reasons = left_out_because(&other_release);
    assert!(
        reasons
            .iter()
            .any(|reason| reason.starts_with("the record is for manifest")),
        "{reasons:?}"
    );

    let mut record = qualified_record();
    for application in record["run"]["applications"]
        .as_array_mut()
        .expect("applications")
    {
        if application["version"] == "2.1.278" {
            application["sha256"] = json!(CLAUDE_CODE_NEWER_SHA256);
        }
    }
    let other_build = temporary.path().join("other-build");
    claude_code_repository(&other_build, &record);
    assert_eq!(
        left_out_because(&other_build),
        vec![format!(
            "the record's parts ran against claude-code 2.1.278 with the SHA-256 \
             {CLAUDE_CODE_NEWER_SHA256} (installed), not claude-code 2.1.278 with the SHA-256 \
             {CLAUDE_CODE_SHA256}"
        )]
    );

    let mut record = qualified_record();
    record["run"]["applications"][0]["status"] = json!("not_installed");
    let not_installed = temporary.path().join("not-installed");
    claude_code_repository(&not_installed, &record);
    assert_eq!(
        left_out_because(&not_installed),
        vec![format!(
            "the record's parts ran against claude-code 2.1.278 with the SHA-256 \
             {CLAUDE_CODE_SHA256} (not_installed), not claude-code 2.1.278 with the SHA-256 \
             {CLAUDE_CODE_SHA256}"
        )]
    );

    let mut record = qualified_record();
    let own_home = record["steps"]
        .as_array_mut()
        .expect("steps")
        .iter_mut()
        .find(|step| step["group"] == "own-home")
        .expect("the own-home check");
    own_home["exit"] = json!(1);
    own_home["error"] = json!("the session named no login keychain");
    record["failures_outside_identifiers"] =
        json!([{ "step": 1, "failure": "the session named no login keychain" }]);
    recount(&mut record);
    let own_home_failed = temporary.path().join("own-home-failed");
    claude_code_repository(&own_home_failed, &record);
    assert_eq!(
        left_out_because(&own_home_failed),
        vec![
            "the run has failures outside its identifiers".to_owned(),
            "steps 1 did not exit cleanly".to_owned(),
        ]
    );

    let mut record = qualified_record();
    record["problems"] = json!(["a record a test wrote could not be read"]);
    let with_problems = temporary.path().join("with-problems");
    claude_code_repository(&with_problems, &record);
    assert_eq!(
        left_out_because(&with_problems),
        vec!["the run names problems that leave its result incomplete".to_owned()]
    );

    let without_record = temporary.path().join("without-record");
    claude_code_repository(&without_record, &qualified_record());
    std::fs::remove_file(without_record.join(CLAUDE_CODE_RECORD)).expect("the record goes");
    assert_eq!(
        left_out_because(&without_record),
        vec![format!("no qualification record at {CLAUDE_CODE_RECORD}")]
    );
}

/// A pin names an executable only when its file is what a process runs: a script's process is its
/// interpreter and a wheel is an archive, so neither is named, whatever its record shows.
#[test]
fn a_script_or_a_wheel_pin_names_no_executable() {
    for (launch, reason) in [
        (
            "script",
            "the pinned file is a script, whose process is its interpreter, so it names no \
             executable a host runs",
        ),
        (
            "wheel",
            "the pinned file is a wheel, an archive, so it names no executable a host runs",
        ),
    ] {
        let temporary = tempfile::tempdir().expect("a temporary directory");
        let repository = temporary.path().join("repository");
        claude_code_repository(&repository, &qualified_record());
        let mut entry = claude_code_pin();
        entry["launch"] = json!(launch);
        write_list(&repository, &[entry]);
        assert_eq!(left_out_because(&repository), vec![reason.to_owned()]);
    }
}

/// Input that cannot be read for what it shows is refused rather than left out: a record in bad
/// form, a record whose run is not on the platform its name says, a build list naming one
/// executable as two versions, a platform the pipeline does not know, a build pinned twice, and a
/// build pinned for a package the repository does not publish.
#[test]
fn input_that_cannot_be_read_for_what_it_shows_is_refused() {
    let temporary = tempfile::tempdir().expect("a temporary directory");

    let mut record = qualified_record();
    let tests = record["identifiers"]["KR-REQ-12.32"]["tests"]
        .as_array_mut()
        .expect("tests");
    let again = tests[0].clone();
    tests.push(again);
    let bad_form = temporary.path().join("bad-form");
    claude_code_repository(&bad_form, &record);
    let refusal = refused(&bad_form);
    assert!(
        refusal.contains("is not a qualification record in good form")
            && refusal.contains("names part 1 2 times"),
        "{refusal}"
    );

    let mut record = qualified_record();
    record["run"]["system"]["arch"] = json!("x86_64");
    let other_platform = temporary.path().join("other-platform");
    claude_code_repository(&other_platform, &record);
    let refusal = refused(&other_platform);
    assert!(
        refusal.contains("the run is not an aarch64 run"),
        "{refusal}"
    );

    // A record that names no whole release, or no list of builds, is not a record for another
    // release or build: it names neither, and is refused rather than read as one.
    let mut record = qualified_record();
    record["run"]["packages"] = json!([{}]);
    let no_release = temporary.path().join("no-release");
    claude_code_repository(&no_release, &record);
    let refusal = refused(&no_release);
    assert!(
        refusal.contains(
            "run.packages names {} without a package, a release version and a manifest digest"
        ),
        "{refusal}"
    );
    let mut record = qualified_record();
    record["run"]["applications"] = json!({});
    let no_builds = temporary.path().join("no-builds");
    claude_code_repository(&no_builds, &record);
    let refusal = refused(&no_builds);
    assert!(
        refusal.contains(
            "run.applications is not a list whose first entry is the build the parts ran against"
        ),
        "{refusal}"
    );
    let mut record = qualified_record();
    record["run"]["applications"][0]
        .as_object_mut()
        .expect("the tested build")
        .remove("sha256");
    let no_digest = temporary.path().join("no-digest");
    claude_code_repository(&no_digest, &record);
    let refusal = refused(&no_digest);
    assert!(
        refusal.contains("without an application, a version, a SHA-256 and a status"),
        "{refusal}"
    );

    let two_versions = temporary.path().join("two-versions");
    claude_code_repository(&two_versions, &qualified_record());
    let mut entry = claude_code_pin();
    entry["newer"]["sha256"] = json!(CLAUDE_CODE_SHA256);
    write_list(&two_versions, &[entry]);
    let refusal = refused(&two_versions);
    assert!(
        refusal.contains(&format!(
            "the executable {CLAUDE_CODE_SHA256} is named as version 2.1.278 and as version 2.1.281"
        )),
        "{refusal}"
    );

    let unknown_platform = temporary.path().join("unknown-platform");
    claude_code_repository(&unknown_platform, &qualified_record());
    let mut list = read_json(&unknown_platform.join(builds::LIST));
    list["platform"] = json!("macos-riscv64");
    write_json(&unknown_platform.join(builds::LIST), &list);
    let refusal = refused(&unknown_platform);
    assert!(
        refusal.contains("names the architecture \"riscv64\""),
        "{refusal}"
    );

    let pinned_twice = temporary.path().join("pinned-twice");
    claude_code_repository(&pinned_twice, &qualified_record());
    let twice = claude_code_pin();
    write_list(&pinned_twice, &[twice.clone(), twice]);
    let refusal = refused(&pinned_twice);
    assert!(
        refusal.contains("it pins kalareach/claude-code 2.1.278 twice"),
        "{refusal}"
    );

    let unpublished = temporary.path().join("unpublished");
    claude_code_repository(&unpublished, &qualified_record());
    write_list(
        &unpublished,
        &[
            claude_code_pin(),
            pin(
                "kalareach/codex",
                "codex",
                "0.155.1",
                "npm @openai/codex",
                "child",
                CODEX_SHA256,
                None,
            ),
        ],
    );
    let refusal = refused(&unpublished);
    assert!(
        refusal
            .contains("pins a build for kalareach/codex, which this repository does not publish"),
        "{refusal}"
    );

    let no_connector = temporary.path().join("no-connector");
    claude_code_repository(&no_connector, &qualified_record());
    add_example_package(&no_connector);
    write_list(&no_connector, &[claude_code_pin(), example_pin()]);
    let refusal = refused(&no_connector);
    assert!(
        refusal.contains(
            "pins a build for kalareach/example-declarative, whose release has no connector table \
             for a build to be qualified against"
        ),
        "{refusal}"
    );
}

/// One executable pinned for two packages at one version, as OpenCode's two packages pin theirs, is
/// one executable at one version: the list is read, and each pin is decided on its own record.
#[test]
fn one_executable_pinned_for_two_packages_at_one_version_is_read() {
    let temporary = tempfile::tempdir().expect("a temporary directory");
    let repository = temporary.path().join("repository");
    std::fs::create_dir_all(repository.join("publishers")).expect("publishers directory");
    std::fs::copy(
        root().join("publishers/kalareach.json"),
        repository.join("publishers/kalareach.json"),
    )
    .expect("the publisher record copies");
    for plugin in ["opencode", "opencode-attach"] {
        copy_tree(
            &root().join("plugins/kalareach").join(plugin),
            &repository.join("plugins/kalareach").join(plugin),
        );
    }
    write_list(
        &repository,
        &["opencode", "opencode-attach"].map(|plugin| {
            pin(
                &format!("kalareach/{plugin}"),
                plugin,
                "1.18.31",
                "npm opencode-ai",
                "native",
                OPENCODE_SHA256,
                None,
            )
        }),
    );

    let list = builds::read_list(&repository)
        .expect("the build list reads")
        .expect("the repository has a build list");
    assert_eq!(list.builds.len(), 2);
    assert_eq!(list.builds[0].sha256, list.builds[1].sha256);
    assert_eq!(list.builds[0].version, list.builds[1].version);
    let loaded = packages::load(&repository).expect("the repository loads");
    let omitted = loaded.repository.builds.omitted();
    assert_eq!(omitted.len(), 2, "{omitted:?}");
    assert!(
        omitted.iter().all(|omitted| omitted.reasons.len() == 1
            && omitted.reasons[0].starts_with("no qualification record at")),
        "{omitted:?}"
    );
}

/// Where the record of the newer Claude Code build is, beside the pinned build's.
const NEWER_RECORD: &str = "fixtures/agents/kalareach/claude-code/2.1.281/macos-aarch64.json";

/// A repository whose list pins the newer Claude Code build as well as the pinned one, newer first,
/// with `newer_record` as the newer build's record.
fn two_build_repository(directory: &Path, newer_record: &Value) {
    claude_code_repository(directory, &qualified_record());
    let first = claude_code_pin();
    let mut second = first.clone();
    second["version"] = json!("2.1.281");
    second["sha256"] = json!(CLAUDE_CODE_NEWER_SHA256);
    second["newer"] = Value::Null;
    write_list(directory, &[second, first]);
    write_json(&directory.join(NEWER_RECORD), newer_record);
}

/// The newer Claude Code build, as an index entry names it.
fn newer_build() -> QualifiedBuild {
    let mut newer = claude_code_build();
    newer.version = PackageVersion::parse("2.1.281").expect("a version");
    newer.executable_digest = PayloadDigest::parse(CLAUDE_CODE_NEWER_SHA256).expect("a digest");
    newer
}

/// A record of the newer Claude Code build's own run, with every part passed: the build its parts
/// ran against, first in its applications, is the newer build.
fn newer_build_record() -> Value {
    let mut record = qualified_record();
    record["run"]["applications"] = json!([{
        "id": "claude-code",
        "version": "2.1.281",
        "status": "installed",
        "url": "https://registry.npmjs.org/@anthropic-ai/claude-code-darwin-arm64/-/claude-code-darwin-arm64-2.1.281.tgz",
        "sha256": CLAUDE_CODE_NEWER_SHA256,
        "build": "darwin-arm64",
        "reason": "the build the table is pinned to",
    }]);
    record
}

/// A generation adds a build when another pinned build's own record qualifies it, with no change to
/// the package: a record whose parts ran against that build, first in its applications. An entry
/// writes its builds in one order, whatever order the list gives them in.
#[test]
fn a_second_build_whose_own_record_qualifies_it_is_added_in_the_entry_s_order() {
    let temporary = tempfile::tempdir().expect("a temporary directory");
    let repository = temporary.path().join("repository");
    let newer_record = newer_build_record();
    two_build_repository(&repository, &newer_record);

    assert_eq!(
        claude_code_entry_builds(&repository),
        vec![claude_code_build(), newer_build()]
    );
    let loaded = packages::load(&repository).expect("the repository loads");
    assert_eq!(
        builds::check_records(&repository, &loaded.repository.packages).expect("the records read"),
        Vec::<String>::new()
    );
    assert_eq!(
        index_of(&loaded.repository)
            .canonical_json()
            .expect("the index renders"),
        index_of(&loaded.repository)
            .canonical_json()
            .expect("the index renders")
    );
}

/// The newer build an upgrade part moves to is listed in the record of the build it upgrades from,
/// after it, and that record's parts did not run against it: the same record under the newer
/// build's name qualifies nothing, and only the build it ran against is named.
#[test]
fn the_upgrade_target_listed_in_another_build_s_record_is_not_named() {
    let temporary = tempfile::tempdir().expect("a temporary directory");
    let repository = temporary.path().join("repository");
    two_build_repository(&repository, &qualified_record());

    let loaded = packages::load(&repository).expect("the repository loads");
    assert_eq!(
        claude_code_entry(&mut index_of(&loaded.repository)).builds,
        vec![claude_code_build()]
    );
    let omitted = loaded.repository.builds.omitted();
    assert_eq!(omitted.len(), 1, "{omitted:?}");
    assert_eq!(omitted[0].version, "2.1.281");
    assert_eq!(
        omitted[0].reasons,
        vec![format!(
            "the record's parts ran against claude-code 2.1.278 with the SHA-256 \
             {CLAUDE_CODE_SHA256} (installed), not claude-code 2.1.281 with the SHA-256 \
             {CLAUDE_CODE_NEWER_SHA256}"
        )]
    );
    let problems =
        builds::check_records(&repository, &loaded.repository.packages).expect("the records read");
    assert!(
        problems
            .iter()
            .any(|problem| problem.starts_with(NEWER_RECORD)
                && problem.contains("parts ran against claude-code 2.1.278")),
        "{problems:?}"
    );
}

/// A build is withdrawn by removing its pin: the next index leaves it out, the package and its
/// record stay as they are, and the record still meets every check the repository makes.
#[test]
fn removing_a_pin_withdraws_its_build_and_leaves_the_record_whole() {
    let temporary = tempfile::tempdir().expect("a temporary directory");
    let repository = temporary.path().join("repository");
    claude_code_repository(&repository, &qualified_record());
    assert_eq!(
        claude_code_entry_builds(&repository),
        vec![claude_code_build()]
    );

    write_list(&repository, &[]);
    let loaded = packages::load(&repository).expect("the repository loads");
    assert_eq!(loaded.repository.builds.named().count(), 0);
    assert!(loaded.repository.builds.omitted().is_empty());
    assert!(
        claude_code_entry(&mut index_of(&loaded.repository))
            .builds
            .is_empty()
    );
    assert!(repository.join(CLAUDE_CODE_RECORD).is_file());
    assert_eq!(
        builds::check_records(&repository, &loaded.repository.packages).expect("the records read"),
        Vec::<String>::new()
    );
}

/// A build no connector table names is withdrawn the same way: its pin goes, its record stays and
/// still meets every check the repository makes, and the index names the builds still pinned.
#[test]
fn removing_the_pin_of_a_build_no_table_names_leaves_its_record_whole() {
    let temporary = tempfile::tempdir().expect("a temporary directory");
    let repository = temporary.path().join("repository");
    two_build_repository(&repository, &newer_build_record());
    assert_eq!(
        claude_code_entry_builds(&repository),
        vec![claude_code_build(), newer_build()]
    );

    write_list(&repository, &[claude_code_pin()]);
    let loaded = packages::load(&repository).expect("the repository loads");
    assert_eq!(
        claude_code_entry(&mut index_of(&loaded.repository)).builds,
        vec![claude_code_build()]
    );
    assert!(repository.join(NEWER_RECORD).is_file());
    assert_eq!(
        builds::check_records(&repository, &loaded.repository.packages).expect("the records read"),
        Vec::<String>::new()
    );
}

/// A record is for a connector package: a record in good form for the release of a package with no
/// connector table, which no build could be qualified by, is one the record check reports.
#[test]
fn a_record_for_a_package_without_a_connector_is_reported() {
    let temporary = tempfile::tempdir().expect("a temporary directory");
    let repository = temporary.path().join("repository");
    claude_code_repository(&repository, &qualified_record());
    let digest = add_example_package(&repository);
    let mut record = qualified_record();
    record["run"]["packages"] = json!([{
        "name": "kalareach/example-declarative",
        "version": "0.1.0",
        "manifest_digest": digest,
        "generation": "snapshots/development",
    }]);
    record["attachment_paths"] = json!([{
        "operation": "terminal",
        "declared": null,
        "source": "plugin.json",
        "package_digest": digest,
    }]);
    record["run"]["applications"] = json!([{
        "id": "example-agent",
        "version": "1.0.0",
        "status": "installed",
        "url": "https://registry.npmjs.org/@kalareach/example-agent/-/example-agent-1.0.0.tgz",
        "sha256": "a".repeat(64),
        "build": "darwin-arm64",
        "reason": "the build the table is pinned to",
    }]);
    let relative = "fixtures/agents/kalareach/example-declarative/1.0.0/macos-aarch64.json";
    write_json(&repository.join(relative), &record);

    let loaded = packages::load(&repository).expect("the repository loads");
    assert_eq!(
        builds::check_records(&repository, &loaded.repository.packages).expect("the records read"),
        vec![format!(
            "{relative}: a record for kalareach/example-declarative, whose release has no \
             connector table for a build to be qualified against"
        )]
    );
}

/// A signing directory with no keys in it, outside the repository: a build that is refused before
/// it reads a key never finds out.
fn keyless(temporary: &Path, repository: &Path) -> SigningDirectory {
    let keys = temporary.join("keys");
    std::fs::create_dir_all(&keys).expect("the directory creates");
    SigningDirectory::open(&keys, repository).expect("the signing directory opens")
}

fn expiries() -> Expiries {
    let expires = Timestamp::now()
        .checked_add(Span::new().hours(24))
        .expect("an hour arithmetic that fits");
    Expiries {
        targets: expires,
        snapshot: expires,
        timestamp: expires,
    }
}

/// Builds `index` into a new directory and says why the build was refused, after checking that it
/// wrote nothing: no generation and no directory it assembles one in.
async fn refused_before_signing(
    repository: &Repository,
    index: &CatalogueIndex,
    signing: &SigningDirectory,
    temporary: &Path,
) -> Error {
    let out = temporary.join("generation");
    let refusal = tuf::build(repository, index, signing, expiries(), &out, false)
        .await
        .expect_err("the generation is refused");
    assert!(!out.exists(), "the generation was written");
    assert!(
        !temporary.join(".generation.building").exists(),
        "a generation was assembled"
    );
    refusal
}

/// The pipeline refuses what the SDK refuses before it signs: an entry naming more builds than one
/// entry may, and one naming one executable as two versions. The bound itself is allowed.
#[tokio::test]
async fn the_pipeline_refuses_too_many_builds_and_two_versions_of_one_executable_before_it_signs() {
    let temporary = tempfile::tempdir().expect("a temporary directory");
    let directory = temporary.path().join("repository");
    claude_code_repository(&directory, &qualified_record());
    let loaded = packages::load(&directory).expect("the repository loads");
    let signing = keyless(temporary.path(), &directory);

    let mut at_bound = index_of(&loaded.repository);
    claude_code_entry(&mut at_bound).builds = (0..MAX_QUALIFIED_BUILDS)
        .map(|n| {
            let mut build = claude_code_build();
            build.executable_digest = PayloadDigest::of(&n.to_le_bytes());
            build
        })
        .collect();
    index::check_builds(&at_bound).expect("the bound is allowed");

    let mut too_many = at_bound.clone();
    claude_code_entry(&mut too_many)
        .builds
        .push(claude_code_build());
    let refusal =
        refused_before_signing(&loaded.repository, &too_many, &signing, temporary.path()).await;
    assert!(
        matches!(
            &refusal,
            Error::Builds { plugin_id, source, .. }
                if plugin_id == "kalareach/claude-code"
                    && **source == BuildsError::TooMany { count: MAX_QUALIFIED_BUILDS + 1 }
        ),
        "{refusal}"
    );
    assert_eq!(
        refusal.to_string(),
        format!(
            "kalareach/claude-code 0.5.0: the entry names {} builds, and one entry names at most \
             {MAX_QUALIFIED_BUILDS}",
            MAX_QUALIFIED_BUILDS + 1
        )
    );

    let mut two_versions = index_of(&loaded.repository);
    let mut again = claude_code_build();
    again.version = PackageVersion::parse("2.1.281").expect("a version");
    claude_code_entry(&mut two_versions).builds.push(again);
    let refusal = refused_before_signing(
        &loaded.repository,
        &two_versions,
        &signing,
        temporary.path(),
    )
    .await;
    assert_eq!(
        refusal.to_string(),
        format!(
            "kalareach/claude-code 0.5.0: the executable {CLAUDE_CODE_SHA256} is named as version \
             2.1.278 and as version 2.1.281"
        )
    );
}

/// A record for a release that does not list the platform its build runs on is a build the SDK
/// refuses, and the pipeline refuses it before it signs, with the SDK's words.
#[tokio::test]
async fn the_pipeline_refuses_a_build_on_a_platform_the_release_does_not_list_before_it_signs() {
    let temporary = tempfile::tempdir().expect("a temporary directory");
    let directory = temporary.path().join("repository");
    claude_code_repository(&directory, &qualified_record());

    // The release lists Linux alone. Its manifest is another document, so the record is made for
    // that manifest.
    let manifest_path = directory.join("plugins/kalareach/claude-code/plugin.json");
    let mut manifest = read_json(&manifest_path);
    manifest["platforms"] = json!([{ "os": "linux", "architectures": ["x86_64", "aarch64"] }]);
    write_json(&manifest_path, &manifest);
    let digest =
        PayloadDigest::of(&std::fs::read(&manifest_path).expect("the manifest reads")).to_string();
    let mut record = qualified_record();
    record["run"]["packages"][0]["manifest_digest"] = json!(digest);
    for path in record["attachment_paths"]
        .as_array_mut()
        .expect("attachment paths")
    {
        path["package_digest"] = json!(digest);
    }
    write_json(&directory.join(CLAUDE_CODE_RECORD), &record);

    let loaded = packages::load(&directory).expect("the repository loads");
    let mut index = index_of(&loaded.repository);
    assert_eq!(
        claude_code_entry(&mut index).builds,
        vec![claude_code_build()]
    );
    let signing = keyless(temporary.path(), &directory);
    let refusal =
        refused_before_signing(&loaded.repository, &index, &signing, temporary.path()).await;
    assert_eq!(
        refusal.to_string(),
        "kalareach/claude-code 0.5.0: a build runs on mac_os aarch64, which the entry does not \
         list among its platforms"
    );
}

/// Signs `index` over the repository's payloads into `out`, the way a build does but without its
/// checks: what a generation another builder signed could carry.
async fn signed_without_checks(
    repository: &Repository,
    index: &CatalogueIndex,
    signing: &SigningDirectory,
    out: &Path,
) {
    let staged =
        tuf::stage_targets(repository, index, &out.join(TARGETS_DIR)).expect("the targets stage");
    let root_bytes = std::fs::read(signing.root_path()).expect("the trust root reads");
    std::fs::write(out.join("root.json"), &root_bytes).expect("the trust root writes");
    let version = std::num::NonZeroU64::new(1).expect("one is not zero");
    let expiries = expiries();
    let mut editor = tough::editor::RepositoryEditor::new(&out.join("root.json"))
        .await
        .expect("the editor opens");
    editor
        .targets_version(version)
        .expect("the targets version")
        .targets_expires(expiries.targets)
        .expect("the targets expiry")
        .snapshot_version(version)
        .snapshot_expires(expiries.snapshot)
        .timestamp_version(version)
        .timestamp_expires(expiries.timestamp);
    for target in &staged {
        let built = tough::schema::Target::from_path(&target.path)
            .await
            .expect("the target reads");
        editor
            .add_target(target.name.as_str(), built)
            .expect("the target is added");
    }
    let signed = editor
        .sign(&signing.key_sources().expect("the keys read"))
        .await
        .expect("the metadata signs");
    signed
        .write(out.join(METADATA_DIR))
        .await
        .expect("the metadata writes");
    std::fs::write(out.join(METADATA_DIR).join("root.json"), &root_bytes)
        .expect("the trust root writes");
}

/// Verification refuses a generation whose index names builds the SDK refuses, as a host does,
/// even when its signatures hold.
#[tokio::test]
async fn verification_refuses_a_signed_generation_whose_builds_the_sdk_refuses() {
    let Some(signing_dir) = std::env::var_os("KALAREACH_SIGNING_DIR") else {
        return;
    };
    let loaded = packages::load(&root()).expect("the repository loads");
    let signing = SigningDirectory::open(Path::new(&signing_dir), &root())
        .expect("the signing directory opens");
    let mut index = index_of(&loaded.repository);
    let mut unlisted = claude_code_build();
    unlisted.os = OperatingSystem::Windows;
    unlisted.architecture = Architecture::Aarch64;
    claude_code_entry(&mut index).builds.push(unlisted);

    let temporary = tempfile::tempdir().expect("a temporary directory");
    let out = temporary.path().join("generation");
    signed_without_checks(&loaded.repository, &index, &signing, &out).await;
    let refusal = tuf::verify(&out, true)
        .await
        .expect_err("the generation is refused");
    assert_eq!(
        refusal.to_string(),
        "kalareach/claude-code 0.5.0: a build runs on windows aarch64, which the entry does not \
         list among its platforms"
    );
}
