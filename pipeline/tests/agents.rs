//! Every connector package has a qualification record for the build its table is pinned to.
//!
//! A connector table is qualified against one upstream build, and section 12 asks for eight cases
//! to be run against that build and recorded per operating system and architecture. The records are
//! under `fixtures/agents/<publisher>/<plugin>/<version>/<os>-<arch>.json`, in the form
//! `fixtures/agents/README.md` states. This reads each package's pinned build out of its own table
//! and requires the record for it: bound to this release of the package by its manifest digest,
//! naming every part of every case once with an outcome it can carry, and declaring the attachment
//! paths the manifest declares. A record no package pins is a stale record, and fails the same way.
//!
//! What this cannot say is whether a case qualifies: a record that names a part as not run is still
//! a record. Acceptance is read from the outcomes, part by part.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use kalareach_catalogue::packages::LoadedPackage;
use kalareach_catalogue::{packages, repository_root};
use kr_plugin_sdk::effect::{AttachmentInsertion, EffectClass};
use serde_json::{Value, json};

/// Where the records are, relative to the repository root.
const RECORDS: &str = "fixtures/agents";

/// The operating system and architecture the records of this repository are run on.
const PLATFORM: &str = "macos-aarch64";

/// The parts of section 12's eight qualification cases, each named once by a record.
const CASE_PARTS: [&str; 13] = [
    "1", "2a", "2b", "2c", "3", "4", "5a", "5b", "6a", "6b", "7", "8a", "8b",
];

/// The parts of the row that keeps a typed path from counting as an accepted attachment.
const ATTACHMENT_PARTS: [&str; 2] = ["14.03a", "14.03b"];

/// The outcomes a part can have.
const OUTCOMES: [&str; 3] = ["passed", "failed", "not_run"];

fn root() -> PathBuf {
    repository_root(Path::new(env!("CARGO_MANIFEST_DIR"))).expect("the repository root is above us")
}

#[test]
fn every_connector_package_has_a_qualification_record_for_its_pinned_build() {
    let root = root();
    let loaded = packages::load(&root).expect("the repository loads");
    let mut problems = Vec::new();
    let mut pinned = BTreeSet::new();
    for package in &loaded.repository.packages {
        let Some(connector) = &package.package.connector else {
            continue;
        };
        let manifest = &package.package.manifest;
        let tested = connector.protocol.tested_version.to_string();
        let relative = Path::new(RECORDS)
            .join(manifest.publisher_id.as_str())
            .join(manifest.plugin_name.as_str())
            .join(&tested)
            .join(format!("{PLATFORM}.json"));
        pinned.insert(relative.clone());
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
                let mut found = Vec::new();
                check_record(&record, package, &tested, &mut found);
                problems.extend(
                    found
                        .into_iter()
                        .map(|problem| format!("{}: {problem}", relative.display())),
                );
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

/// Checks one record against the package it is for, and says everything that does not hold.
fn check_record(record: &Value, package: &LoadedPackage, tested: &str, problems: &mut Vec<String>) {
    let manifest = &package.package.manifest;
    let digest = package.manifest_digest.to_string();
    for (field, wanted) in [
        ("schema", "kalareach.conformance/1"),
        ("extension", "kalareach.agent-qualification/1"),
        ("repository", "kalareach-plugins"),
    ] {
        if record[field] != wanted {
            problems.push(format!("{field} is {}, not {wanted:?}", record[field]));
        }
    }
    let run = &record["run"];
    if run["platform"] != "macos" || run["system"]["os"] != "macos" {
        problems.push("the run is not a macOS run".to_owned());
    }
    if run["system"]["arch"] != "aarch64" {
        problems.push("the run is not an aarch64 run".to_owned());
    }
    for (what, commit) in [
        ("run.commit", &run["commit"]),
        ("run.host.commit", &run["host"]["commit"]),
    ] {
        if !is_hex(&commit["id"], 40) || !commit["modified"].is_boolean() {
            problems.push(format!(
                "{what} names no commit and whether it was modified"
            ));
        }
    }
    if run["host"]["repository"] != "kalareach" {
        problems.push("run.host names no host repository".to_owned());
    }
    let named: Vec<&Value> = entries(&run["packages"])
        .into_iter()
        .filter(|entry| entry["name"] == manifest.plugin_id().to_string())
        .collect();
    match named.as_slice() {
        [entry] => {
            if entry["version"] != manifest.version.to_string() {
                problems.push(format!(
                    "the record is for release {}, and the package is {}",
                    entry["version"], manifest.version
                ));
            }
            if entry["manifest_digest"] != digest {
                problems.push(format!(
                    "the record is for manifest {}, and the package's is {digest}",
                    entry["manifest_digest"]
                ));
            }
        }
        _ => problems.push(format!(
            "run.packages names {} once, and it names it {} times",
            manifest.plugin_id(),
            named.len()
        )),
    }
    if !entries(&run["applications"])
        .iter()
        .any(|application| application["version"] == tested && is_hex(&application["sha256"], 64))
    {
        problems.push(format!(
            "run.applications names no build {tested} with its executable's SHA-256"
        ));
    }
    let identifiers = record["identifiers"].as_object();
    let keys: BTreeSet<&str> = identifiers
        .map(|map| map.keys().map(String::as_str).collect())
        .unwrap_or_default();
    if keys != BTreeSet::from(["KR-REQ-12.32", "KR-REQ-14.03"]) {
        problems.push(format!(
            "the identifiers are {keys:?}, not KR-REQ-12.32 and KR-REQ-14.03"
        ));
    }
    let tests = check_identifier(record, "KR-REQ-12.32", &CASE_PARTS, problems);
    let more = check_identifier(record, "KR-REQ-14.03", &ATTACHMENT_PARTS, problems);
    let names: BTreeSet<String> = tests.into_iter().chain(more).collect();
    for test in record["identifiers"]
        .as_object()
        .into_iter()
        .flat_map(|map| map.values())
        .flat_map(|identifier| entries(&identifier["tests"]))
    {
        let condition = &test["condition"];
        if condition.is_null() {
            continue;
        }
        let fallback = condition["fallback"].as_str().unwrap_or_default();
        if test["outcome"] != "not_run"
            || condition["gate"].as_str().is_none_or(str::is_empty)
            || !["untested", "unavailable"].contains(&condition["observed"].as_str().unwrap_or(""))
            || !names.contains(fallback)
        {
            problems.push(format!(
                "{}: a gated test stays not run and names its gate, what was observed of it and \
                 a fallback test the record holds",
                test["test"]
            ));
        }
    }
    let declared = attachment_paths(package);
    if record["attachment_paths"] != declared {
        problems.push(format!(
            "attachment_paths is {}, and the manifest declares {declared}",
            record["attachment_paths"]
        ));
    }
    for list in [
        "steps",
        "failures_outside_identifiers",
        "known_differences",
        "problems",
        "warnings",
    ] {
        if !record[list].is_array() {
            problems.push(format!("{list} is not a list"));
        }
    }
    if run["terminal_profile"]["profile"]
        .as_str()
        .is_none_or(str::is_empty)
        || run["terminal_profile"]["term"]
            .as_str()
            .is_none_or(str::is_empty)
    {
        problems.push("run.terminal_profile names no profile and terminal".to_owned());
    }
    check_steps(record, problems);
    check_summary(record, problems);
}

/// Checks the steps: each one the run ran, with its command as run, its exit status and its log;
/// the own-home check first, and a failure outside the identifiers exactly when it failed; and
/// every test that ran naming a part's step, with that step's command and outcome.
fn check_steps(record: &Value, problems: &mut Vec<String>) {
    let steps = entries(&record["steps"]);
    let mut numbers = BTreeSet::new();
    for step in &steps {
        let number = step["number"].as_u64();
        if !number.is_some_and(|number| numbers.insert(number))
            || step["command"].as_str().is_none_or(str::is_empty)
            || step["what"].as_str().is_none_or(str::is_empty)
            || step["log"].as_str().is_none_or(str::is_empty)
            || !step["exit"].is_i64()
            || !step["seconds"].is_u64()
            || !step["needs"].is_array()
            || !(step["error"].is_null() || step["error"].is_string())
        {
            problems.push(format!(
                "step {} names no distinct number, command, log, exit status, time, needs and error",
                step["number"]
            ));
        }
    }
    let own_home: Vec<&&Value> = steps
        .iter()
        .filter(|step| step["group"] == "own-home")
        .collect();
    match own_home.as_slice() {
        [step] => {
            // The check failed when it exited otherwise than 0, said why, or read no keychain.
            let failed = step["exit"] != 0
                || !step["error"].is_null()
                || record["run"]["own_home"]["default_keychain"]
                    .as_str()
                    .is_none();
            if failed == entries(&record["failures_outside_identifiers"]).is_empty() {
                problems.push(
                    "failures_outside_identifiers names the own-home check exactly when it failed"
                        .to_owned(),
                );
            }
        }
        _ => problems.push(format!(
            "the steps hold one own-home check, and they hold {}",
            own_home.len()
        )),
    }
    for test in record["identifiers"]
        .as_object()
        .into_iter()
        .flat_map(|map| map.values())
        .flat_map(|identifier| entries(&identifier["tests"]))
    {
        let part_step = format!("part {}", test["part"].as_str().unwrap_or_default());
        for run in entries(&test["runs"]) {
            let step = steps.iter().find(|step| step["number"] == run["step"]);
            match step {
                Some(step)
                    if step["group"] == "agents"
                        && step["what"] == part_step.as_str()
                        && run["outcome"] == test["outcome"]
                        && test["command"] == step["command"]
                        && (test["outcome"] != "passed"
                            || (step["exit"] == 0 && step["error"].is_null())) => {}
                _ => problems.push(format!(
                    "{}: its run names step {}, which is not its part's step with its command, \
                     its outcome and, for a pass, a clean exit",
                    test["test"], run["step"]
                )),
            }
        }
    }
}

/// Checks that the summary is what the identifiers and the steps make it.
fn check_summary(record: &Value, problems: &mut Vec<String>) {
    let summary = &record["summary"];
    let identifiers: Vec<&Value> = record["identifiers"]
        .as_object()
        .map(|map| map.values().collect())
        .unwrap_or_default();
    let verdicts = |verdict: &str| {
        identifiers
            .iter()
            .filter(|identifier| identifier["verdict"] == verdict)
            .count()
    };
    let mut wanted = json!({
        "identifiers": identifiers.len(),
        "passed": verdicts("passed"),
        "failed": verdicts("failed"),
        "known_difference": verdicts("known_difference"),
        "not_run": verdicts("not_run"),
        "known_differences": entries(&record["known_differences"]).len(),
    });
    let mut tests = serde_json::Map::new();
    for outcome in [
        "passed",
        "failed",
        "ignored",
        "not_run",
        "not_built",
        "known_difference",
    ] {
        let total: u64 = identifiers
            .iter()
            .map(|identifier| identifier["counts"][outcome].as_u64().unwrap_or(0))
            .sum();
        tests.insert(outcome.to_owned(), json!(total));
    }
    wanted["tests"] = Value::Object(tests);
    let failed_steps: Vec<Value> = entries(&record["steps"])
        .into_iter()
        .filter(|step| step["exit"] != 0)
        .map(|step| step["number"].clone())
        .collect();
    wanted["failed_steps"] = Value::Array(failed_steps);
    if *summary != wanted {
        problems.push(format!(
            "summary is {summary}, and the identifiers and steps make it {wanted}"
        ));
    }
}

/// Checks one identifier's tests: each part once, each outcome one a part can have, a reason for
/// a part not run and a command and its runs for a part that ran, and counts and a verdict that
/// agree with them. Returns the names of its tests.
fn check_identifier(
    record: &Value,
    identifier: &str,
    parts: &[&str],
    problems: &mut Vec<String>,
) -> Vec<String> {
    let entry = &record["identifiers"][identifier];
    let tests = entries(&entry["tests"]);
    let mut seen: BTreeMap<&str, usize> = BTreeMap::new();
    let mut counts: BTreeMap<&str, u64> = BTreeMap::new();
    let mut names = Vec::new();
    for test in &tests {
        let name = test["test"].as_str().unwrap_or_default();
        names.push(name.to_owned());
        let part = test["part"].as_str().unwrap_or_default();
        *seen.entry(part).or_default() += 1;
        let outcome = test["outcome"].as_str().unwrap_or_default();
        *counts.entry(outcome).or_default() += 1;
        for field in ["test", "source", "keyed_by"] {
            if test[field].as_str().is_none_or(str::is_empty) {
                problems.push(format!("{identifier} part {part}: {field} is missing"));
            }
        }
        if !OUTCOMES.contains(&outcome) {
            problems.push(format!(
                "{identifier} part {part}: {outcome:?} is not an outcome a part has"
            ));
        }
        if outcome == "not_run" && test["reason"].as_str().is_none_or(str::is_empty) {
            problems.push(format!("{identifier} part {part}: a part not run says why"));
        }
        if outcome != "not_run"
            && (test["command"].as_str().is_none_or(str::is_empty)
                || entries(&test["runs"]).is_empty())
        {
            problems.push(format!(
                "{identifier} part {part}: a part that ran names its command and its runs"
            ));
        }
    }
    for part in parts {
        match seen.get(part) {
            Some(1) => {}
            Some(times) => problems.push(format!("{identifier} names part {part} {times} times")),
            None => problems.push(format!("{identifier} does not name part {part}")),
        }
    }
    for part in seen.keys() {
        if !parts.contains(part) {
            problems.push(format!(
                "{identifier} names part {part:?}, which it has none of"
            ));
        }
    }
    let counted = |outcome: &str| counts.get(outcome).copied().unwrap_or(0);
    for outcome in [
        "passed",
        "failed",
        "ignored",
        "not_run",
        "not_built",
        "known_difference",
    ] {
        if entry["counts"][outcome] != counted(outcome) {
            problems.push(format!(
                "{identifier}: counts.{outcome} is {}, and {} of its tests are {outcome}",
                entry["counts"][outcome],
                counted(outcome)
            ));
        }
    }
    let verdict = if counted("failed") > 0 {
        "failed"
    } else if counted("passed") > 0 {
        "passed"
    } else {
        "not_run"
    };
    if entry["verdict"] != verdict {
        problems.push(format!(
            "{identifier}: the verdict is {}, and its tests make it {verdict}",
            entry["verdict"]
        ));
    }
    names
}

/// The attachment path each operation of the package declares: every action whose effect carries
/// a prompt or an attachment, in the manifest's order, with the insertion the manifest declares,
/// and the terminal route, which the package contract has no way to declare.
fn attachment_paths(package: &LoadedPackage) -> Value {
    let manifest = &package.package.manifest;
    let digest = package.manifest_digest.to_string();
    let declared =
        manifest
            .attachments
            .0
            .as_ref()
            .map(|contribution| match contribution.insertion {
                AttachmentInsertion::UpstreamUpload => "typed_submission",
                AttachmentInsertion::NativeComposer => "verified_composer_insertion",
                AttachmentInsertion::TerminalDraftPath => "terminal_draft_path",
            });
    let mut paths: Vec<Value> = manifest
        .actions
        .iter()
        .filter(|action| {
            matches!(
                action.effect,
                EffectClass::UpstreamPrompt | EffectClass::UpstreamAttachment
            )
        })
        .map(|action| {
            json!({
                "operation": action.id.as_str(),
                "declared": declared,
                "source": "plugin.json",
                "package_digest": digest,
            })
        })
        .collect();
    paths.push(json!({
        "operation": "terminal",
        "declared": null,
        "source": "plugin.json",
        "package_digest": digest,
    }));
    Value::Array(paths)
}

/// The entries of a list, or none where the value is not one.
fn entries(value: &Value) -> Vec<&Value> {
    value
        .as_array()
        .map(|list| list.iter().collect())
        .unwrap_or_default()
}

/// Whether a value is lower-case hexadecimal text of exactly `length` characters.
fn is_hex(value: &Value, length: usize) -> bool {
    value.as_str().is_some_and(|text| {
        text.len() == length
            && text
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    })
}
