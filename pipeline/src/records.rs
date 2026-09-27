//! The agent qualification records.
//!
//! Section 12 asks for eight cases to be run against the upstream build a connector table is
//! qualified for, and recorded per operating system and architecture. The records are under
//! `fixtures/agents/<publisher>/<plugin>/<version>/<os>-<arch>.json`, in the form
//! `fixtures/agents/README.md` states: a conformance result naming every part of every case once,
//! with an outcome it can carry.
//!
//! The checks here come in three kinds, because what a failure means differs. A record that breaks
//! its form says nothing reliable about anything, and the pipeline refuses to build from it. A
//! record in good form can still be for another release of the package or for another build of the
//! application: it says nothing about this one, and the build it would support is left out. What a
//! record in good form shows about each part is what [`crate::builds`] decides a build by.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use kr_plugin_sdk::effect::{AttachmentInsertion, EffectClass};
use kr_plugin_sdk::matching::{Architecture, OperatingSystem};
use serde_json::{Value, json};

use crate::packages::LoadedPackage;

/// Where the records are, relative to the repository root.
pub const RECORDS: &str = "fixtures/agents";

/// The parts of section 12's eight qualification cases, each named once by a record.
pub const CASE_PARTS: [&str; 13] = [
    "1", "2a", "2b", "2c", "3", "4", "5a", "5b", "6a", "6b", "7", "8a", "8b",
];

/// The parts of the row that keeps a typed path from counting as an accepted attachment.
pub const ATTACHMENT_PARTS: [&str; 2] = ["14.03a", "14.03b"];

/// The identifier whose tests are the parts of section 12's cases.
pub const CASES: &str = "KR-REQ-12.32";

/// The identifier whose tests are the parts of the typed-path rule.
pub const ATTACHMENTS: &str = "KR-REQ-14.03";

/// The outcomes a part can have.
const OUTCOMES: [&str; 3] = ["passed", "failed", "not_run"];

/// The operating system and architecture a record was run on, as its file name writes them.
///
/// The name is `<os>-<arch>`: `macos`, `linux` or `windows`, then `aarch64` or `x86_64`. The
/// record's own `run.system` writes them the same way; the index writes the operating system as the
/// SDK does, so `macos` is `mac_os` there.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Platform {
    name: String,
    os_name: String,
    architecture_name: String,
    os: OperatingSystem,
    architecture: Architecture,
}

impl Platform {
    /// Reads a platform name such as `macos-aarch64`.
    ///
    /// # Errors
    ///
    /// Returns what is wrong with a name that is not one operating system and one architecture
    /// this repository records.
    pub fn parse(name: &str) -> Result<Self, String> {
        let Some((os_name, architecture_name)) = name.split_once('-') else {
            return Err(format!(
                "{name:?} is not a platform: a platform is <os>-<arch>, such as macos-aarch64"
            ));
        };
        let os = match os_name {
            "macos" => OperatingSystem::MacOs,
            "linux" => OperatingSystem::Linux,
            "windows" => OperatingSystem::Windows,
            _ => {
                return Err(format!(
                    "{name:?} names the operating system {os_name:?}, and a record is for macos, \
                     linux or windows"
                ));
            }
        };
        let architecture = match architecture_name {
            "aarch64" => Architecture::Aarch64,
            "x86_64" => Architecture::X86_64,
            _ => {
                return Err(format!(
                    "{name:?} names the architecture {architecture_name:?}, and a record is for \
                     aarch64 or x86_64"
                ));
            }
        };
        Ok(Self {
            name: name.to_owned(),
            os_name: os_name.to_owned(),
            architecture_name: architecture_name.to_owned(),
            os,
            architecture,
        })
    }

    /// The name, as a record's file name writes it.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// The operating system, as the index names it.
    #[must_use]
    pub const fn os(&self) -> OperatingSystem {
        self.os
    }

    /// The architecture, as the index names it.
    #[must_use]
    pub const fn architecture(&self) -> Architecture {
        self.architecture
    }
}

impl std::fmt::Display for Platform {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.name)
    }
}

/// Where the record for one release's build of an application on one platform is, relative to the
/// repository root.
#[must_use]
pub fn path(publisher: &str, plugin: &str, version: &str, platform: &Platform) -> PathBuf {
    Path::new(RECORDS)
        .join(publisher)
        .join(plugin)
        .join(version)
        .join(format!("{}.json", platform.name()))
}

/// Everything that breaks the record's form, whatever release and build it is for: its schema, its
/// run on `platform`, its identifiers and their parts, its steps and its summary. A record with any
/// of these cannot be read for what it shows.
#[must_use]
pub fn form_problems(record: &Value, platform: &Platform) -> Vec<String> {
    let mut problems = Vec::new();
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
    if run["platform"] != platform.os_name.as_str()
        || run["system"]["os"] != platform.os_name.as_str()
    {
        problems.push(format!(
            "the run is not a {} run, and the record's file name says it is",
            platform.os_name
        ));
    }
    if run["system"]["arch"] != platform.architecture_name.as_str() {
        problems.push(format!(
            "the run is not an {} run, and the record's file name says it is",
            platform.architecture_name
        ));
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
    if entries(&run["packages"]).is_empty() {
        problems.push("run.packages names no package".to_owned());
    }
    for entry in entries(&run["applications"]) {
        if entry["id"].as_str().is_none_or(str::is_empty)
            || entry["version"].as_str().is_none_or(str::is_empty)
            || !is_hex(&entry["sha256"], 64)
        {
            problems.push(format!(
                "run.applications names {} without an application, a version and a SHA-256",
                entry["id"]
            ));
        }
    }
    let identifiers = record["identifiers"].as_object();
    let keys: BTreeSet<&str> = identifiers
        .map(|map| map.keys().map(String::as_str).collect())
        .unwrap_or_default();
    if keys != BTreeSet::from([CASES, ATTACHMENTS]) {
        problems.push(format!(
            "the identifiers are {keys:?}, not {CASES} and {ATTACHMENTS}"
        ));
    }
    let tests = check_identifier(record, CASES, &CASE_PARTS, &mut problems);
    let more = check_identifier(record, ATTACHMENTS, &ATTACHMENT_PARTS, &mut problems);
    let names: BTreeSet<String> = tests.into_iter().chain(more).collect();
    for test in all_tests(record) {
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
    for list in [
        "steps",
        "failures_outside_identifiers",
        "known_differences",
        "problems",
        "warnings",
        "attachment_paths",
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
    check_steps(record, &mut problems);
    check_summary(record, &mut problems);
    problems
}

/// Where a record in good form is for another release of `package` than the one this repository
/// publishes: another version, another manifest, or the attachment paths another manifest declares.
#[must_use]
pub fn release_problems(record: &Value, package: &LoadedPackage) -> Vec<String> {
    let mut problems = Vec::new();
    let manifest = &package.package.manifest;
    let digest = package.manifest_digest.to_string();
    let named: Vec<&Value> = entries(&record["run"]["packages"])
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
    let declared = attachment_paths(package);
    if record["attachment_paths"] != declared {
        problems.push(format!(
            "attachment_paths is {}, and the manifest declares {declared}",
            record["attachment_paths"]
        ));
    }
    problems
}

/// Where a record in good form names no run of one build: the application, its version and the
/// SHA-256 of its pinned file, together in one of `run.applications`.
#[must_use]
pub fn build_problems(
    record: &Value,
    application: &str,
    version: &str,
    sha256: &str,
) -> Vec<String> {
    let ran = entries(&record["run"]["applications"])
        .into_iter()
        .any(|entry| {
            entry["id"] == application && entry["version"] == version && entry["sha256"] == sha256
        });
    if ran {
        Vec::new()
    } else {
        vec![format!(
            "run.applications names no build {application} {version} with the SHA-256 {sha256}"
        )]
    }
}

/// Every test of every identifier the record holds.
#[must_use]
pub fn all_tests(record: &Value) -> Vec<&Value> {
    record["identifiers"]
        .as_object()
        .into_iter()
        .flat_map(|map| map.values())
        .flat_map(|identifier| entries(&identifier["tests"]))
        .collect()
}

/// The tests of one identifier, by the part each is.
#[must_use]
pub fn parts<'a>(record: &'a Value, identifier: &str) -> BTreeMap<&'a str, &'a Value> {
    entries(&record["identifiers"][identifier]["tests"])
        .into_iter()
        .filter_map(|test| test["part"].as_str().map(|part| (part, test)))
        .collect()
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
    for test in all_tests(record) {
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
