//! Every connector package has a qualification record for the build its table is pinned to.
//!
//! A connector table is qualified against one upstream build, and section 12 asks for eight cases
//! to be run against that build and recorded per operating system and architecture. The records are
//! under `fixtures/agents/<publisher>/<plugin>/<version>/<os>-<arch>.json`, in the form
//! `fixtures/agents/README.md` states. `kalareach_catalogue::builds::check_records` requires the
//! record of the build each table is pinned to, and holds every record to that form: for a package
//! this repository publishes, bound to its current release by the manifest digest, and run against
//! the build its path names, with the SHA-256 the build list pins where it pins that build. A record
//! kept after its build's pin is removed is held to the same.
//!
//! What this cannot say is whether a case qualifies: a record that names a part as not run is still
//! a record. Acceptance is read from the outcomes, part by part, and `kalareach_catalogue::builds`
//! is where the index reads them.

use std::path::{Path, PathBuf};

use kalareach_catalogue::{builds, packages, repository_root};
use serde_json::{Value, json};

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

/// Every record under `fixtures/agents`: the JSON files three directories below it.
fn records(root: &Path) -> Vec<(PathBuf, Value)> {
    let mut found = Vec::new();
    let mut pending = vec![(root.join("fixtures").join("agents"), 0)];
    while let Some((directory, depth)) = pending.pop() {
        for entry in std::fs::read_dir(&directory).expect("the directory reads") {
            let path = entry.expect("each entry of the directory reads").path();
            if path.is_dir() && depth < 3 {
                pending.push((path, depth + 1));
            } else if depth == 3
                && path
                    .extension()
                    .is_some_and(|extension| extension == "json")
            {
                let record =
                    serde_json::from_slice(&std::fs::read(&path).expect("the record reads"))
                        .expect("the record is JSON");
                found.push((path, record));
            }
        }
    }
    found.sort_by(|a, b| a.0.cmp(&b.0));
    found
}

/// Every UUID in `text`, as a conversation's identifier is written, in either case: in lower case.
fn conversation_ids(text: &str) -> Vec<String> {
    let shaped = |candidate: &str| {
        candidate.char_indices().all(|(index, character)| {
            if [8, 13, 18, 23].contains(&index) {
                character == '-'
            } else {
                character.is_ascii_hexdigit()
            }
        })
    };
    let mut ids = Vec::new();
    let mut start = 0;
    while start + 36 <= text.len() {
        match text.get(start..start + 36) {
            Some(candidate) if shaped(candidate) => {
                ids.push(candidate.to_ascii_lowercase());
                start += 36;
            }
            _ => start += 1,
        }
    }
    ids
}

/// The part of `text` that names a path in the person's home: all of it from its first "~/".
fn home_region(text: &str) -> Option<&str> {
    text.find("~/").map(|at| &text[at..])
}

/// Every string in `value`.
fn strings<'a>(value: &'a Value, found: &mut Vec<&'a str>) {
    match value {
        Value::String(text) => found.push(text),
        Value::Array(items) => items.iter().for_each(|item| strings(item, found)),
        Value::Object(fields) => fields.values().for_each(|item| strings(item, found)),
        _ => {}
    }
}

/// The parts that run with the login, as the harness names them to the record's writer.
const LOGIN_PARTS: &str = r#"["1", "2a", "3", "4", "7"]"#;

/// `record` as the record's writer, `scripts/record-redaction.jq` run with jq as the harness runs
/// it, publishes it for the build list's `entry` and the account name `user`.
fn written(record: &Value, entry: &Value, user: &str) -> Value {
    let mut child = std::process::Command::new("jq")
        .arg("-L")
        .arg(root().join("scripts"))
        .args(["--arg", "user", user, "--argjson", "entry"])
        .arg(entry.to_string())
        .args(["--argjson", "login_parts", LOGIN_PARTS])
        .arg(r#"include "record-redaction"; redact_record($user; $entry; $login_parts)"#)
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .spawn()
        .expect("jq runs, as the harness that writes the records needs it to");
    std::io::Write::write_all(
        child.stdin.as_mut().expect("jq's input"),
        record.to_string().as_bytes(),
    )
    .expect("the record goes to jq");
    let output = child.wait_with_output().expect("jq ends");
    assert!(output.status.success(), "jq writes the record");
    serde_json::from_slice(&output.stdout).expect("jq writes JSON")
}

/// The build list's entry for the package a record is of, with the ids of the package's actions
/// under `actions` and the capabilities its manifest requests under `grants` as the harness gives
/// them to the writer, or null.
fn entry_of(record: &Value) -> Value {
    let list: Value = serde_json::from_slice(
        &std::fs::read(root().join("fixtures").join("agents").join("builds.json"))
            .expect("the build list reads"),
    )
    .expect("the build list is JSON");
    let package = record["run"]["packages"][0]["name"].as_str().unwrap_or("");
    let Some(mut entry) = list["builds"]
        .as_array()
        .into_iter()
        .flatten()
        .find(|build| build["package"] == package)
        .cloned()
    else {
        return Value::Null;
    };
    let manifest: Value = serde_json::from_slice(
        &std::fs::read(root().join("plugins").join(package).join("plugin.json"))
            .expect("the package's manifest reads"),
    )
    .expect("the manifest is JSON");
    entry["actions"] = manifest["actions"]
        .as_array()
        .into_iter()
        .flatten()
        .map(|action| action["id"].clone())
        .collect();
    entry["grants"] = manifest["capabilities"]
        .as_array()
        .into_iter()
        .flatten()
        .map(|grant| grant["capability"].clone())
        .collect();
    entry
}

/// Every record is as the record's writer publishes it: the writer, applied to it again, changes
/// nothing, so every rule of `scripts/record-redaction.jq` holds of it; and none names the person
/// the ways [`named_beyond_the_run`] looks for.
#[test]
fn every_record_is_as_the_record_writer_publishes_it() {
    let mut problems = Vec::new();
    for (path, record) in records(&root()) {
        let entry = entry_of(&record);
        if written(&record, &entry, "someone") != record {
            problems.push(format!(
                "{}: the record's writer would change it",
                path.display()
            ));
        }
        problems.extend(
            named_beyond_the_run(&record, &entry)
                .into_iter()
                .map(|problem| format!("{}: {problem}", path.display())),
        );
    }
    assert!(
        problems.is_empty(),
        "records not in the published form:\n{}",
        problems.join("\n")
    );
}

/// What a record names of the person beyond what the run needs, where `entry` is the build list's
/// entry for its agent: a managed-preferences path with an account name in it; a change in the
/// person's home named though it names no conversation of the record's own and is none of the
/// files the entry names, or names another conversation; a text whose part naming a path in the
/// home ([`home_region`]) names an identifier that is not one of the record's own conversations;
/// and, in a part that ran with the login where the entry names one, a text of several lines, a
/// server named other than by its placeholder, a screen's rows, or a probe's tools by name.
/// Identifiers are compared in lower case.
fn named_beyond_the_run(record: &Value, entry: &Value) -> Vec<String> {
    let account = &entry["account"];
    let mut problems = Vec::new();
    let mut all = Vec::new();
    strings(record, &mut all);
    for text in &all {
        if let Some((_, rest)) = text.split_once("/Library/Managed Preferences/")
            && rest.contains('/')
            && !rest.starts_with("{user}/")
        {
            problems.push(format!("the account name: {text}"));
        }
    }
    let tests: Vec<&Value> = record["identifiers"]
        .as_object()
        .into_iter()
        .flat_map(|identifiers| identifiers.values())
        .filter_map(|identifier| identifier["tests"].as_array())
        .flatten()
        .collect();
    let fixed: Vec<String> = ["guarded", "shared", "recorded", "append_only", "line_files"]
        .iter()
        .flat_map(|list| account[*list].as_array().into_iter().flatten())
        .filter_map(Value::as_str)
        .map(|file| format!("~/{file}"))
        .collect();
    let mut own: Vec<String> = Vec::new();
    for test in &tests {
        let evidence = &test["evidence"];
        for removed in evidence["person_home"]["created_and_removed"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(Value::as_str)
        {
            own.extend(conversation_ids(removed));
        }
        for file in evidence["person_files"]["appended"]
            .as_array()
            .into_iter()
            .flatten()
        {
            for line in file["own"].as_array().into_iter().flatten() {
                own.extend(
                    line["id"]
                        .as_str()
                        .map(conversation_ids)
                        .unwrap_or_default(),
                );
            }
        }
    }
    for test in &tests {
        let part = test["part"].as_str().unwrap_or("?");
        if let Some(home) = test["evidence"]["person_home"].as_object() {
            for (category, changes) in home {
                if category == "created_and_removed" || category == "others" {
                    continue;
                }
                // The agent's own upkeep is named by paths the build list reports, relative to the
                // agent's data directory, each with counts, a time and a digest: only the paths are
                // names, and each must be one the build list reports.
                if category == "upkeep" {
                    let reported: Vec<&str> = account["confinement"]["reported"]
                        .as_array()
                        .into_iter()
                        .flatten()
                        .filter_map(Value::as_str)
                        .collect();
                    for entry in changes.as_array().into_iter().flatten() {
                        let path = entry["path"].as_str().unwrap_or("");
                        if !reported.contains(&path) {
                            problems.push(format!(
                                "part {part} names a rewritten file its build list does not \
                                 report: {path}"
                            ));
                        }
                    }
                    continue;
                }
                // Entries that say why a path could not be read are never named.
                let annotated = category == "unread_after" || category == "left_unsearched";
                let mut named = Vec::new();
                strings(changes, &mut named);
                for change in named {
                    let ids = conversation_ids(change);
                    let path = if annotated {
                        change.split(": ").next().unwrap_or(change)
                    } else {
                        change
                    };
                    if annotated {
                        problems.push(format!(
                            "part {part} names a path that could not be read under {category}: \
                             {change}"
                        ));
                    } else if ids.iter().any(|id| !own.contains(id)) {
                        problems.push(format!(
                            "part {part} names another conversation under {category}: {change}"
                        ));
                    } else if ids.is_empty() && !fixed.iter().any(|file| file == path) {
                        problems.push(format!(
                            "part {part} names a file its build list does not under {category}: \
                             {change}"
                        ));
                    }
                }
            }
        }
        let login = !account.is_null() && LOGIN_PARTS.contains(&format!("\"{part}\""));
        if login {
            let mut texts = Vec::new();
            strings(test, &mut texts);
            for text in texts {
                if text.contains('\n') {
                    problems.push(format!("part {part} keeps a text of several lines: {text}"));
                }
                if let Some((_, rest)) = text.split_once("mcp_servers.")
                    && !rest.starts_with("<server ")
                {
                    problems.push(format!("part {part} names a server: {text}"));
                }
            }
            let evidence = &test["evidence"];
            for switched_off in evidence["account"]["isolation"]["servers_switched_off"]
                .as_array()
                .into_iter()
                .flatten()
            {
                if !switched_off
                    .as_str()
                    .is_some_and(|name| name.starts_with("<server "))
                {
                    problems.push(format!("part {part} names a server: {switched_off}"));
                }
            }
            for key in ["rows", "local_rows"] {
                if evidence.get(key).is_some() {
                    problems.push(format!("part {part} keeps a screen's rows under {key}"));
                }
            }
            for offered in evidence["account"]["isolation"]["tools_offered"]
                .as_array()
                .into_iter()
                .flatten()
            {
                if !offered["tools"].is_number() {
                    problems.push(format!("part {part} names the tools a probe found offered"));
                }
            }
        }
    }
    for text in &all {
        if text.contains("~/") && text.contains('\n') {
            problems.push(format!("a text of several lines names the home: {text}"));
        } else if home_region(text)
            .is_some_and(|region| conversation_ids(region).iter().any(|id| !own.contains(id)))
        {
            problems.push(format!("a text names another conversation: {text}"));
        }
    }
    problems
}

/// The tests of `record` whose part ran with the person's login, where its entry names one.
fn login_tests<'a>(record: &'a Value, entry: &Value) -> Vec<&'a Value> {
    if entry["account"].is_null() {
        return Vec::new();
    }
    record["identifiers"]
        .as_object()
        .into_iter()
        .flat_map(|identifiers| identifiers.values())
        .filter_map(|identifier| identifier["tests"].as_array())
        .flatten()
        .filter(|test| {
            test["part"]
                .as_str()
                .is_some_and(|part| LOGIN_PARTS.contains(&format!("\"{part}\"")))
        })
        .collect()
}

/// `record` with every text of its login parts' evidence and reason made `word`, and every object
/// of that evidence given a key that is `word` and a value that is.
fn spoiled(record: &Value, entry: &Value, word: &str) -> Value {
    fn spoil(value: &mut Value, word: &str) {
        match value {
            Value::String(text) => *text = word.to_owned(),
            Value::Array(items) => items.iter_mut().for_each(|item| spoil(item, word)),
            Value::Object(fields) => {
                // The servers switched off are the record's own list of names, which the writer
                // publishes as placeholders whatever they are; a word is not one of them.
                fields
                    .iter_mut()
                    .filter(|(key, _)| key.as_str() != "servers_switched_off")
                    .for_each(|(_, item)| spoil(item, word));
                fields.insert(word.to_owned(), Value::String(word.to_owned()));
            }
            _ => {}
        }
    }
    let mut spoiled = record.clone();
    if entry["account"].is_null() {
        return spoiled;
    }
    for identifier in spoiled["identifiers"]
        .as_object_mut()
        .into_iter()
        .flat_map(|identifiers| identifiers.values_mut())
    {
        for test in identifier["tests"].as_array_mut().into_iter().flatten() {
            let login = test["part"]
                .as_str()
                .is_some_and(|part| LOGIN_PARTS.contains(&format!("\"{part}\"")));
            if login {
                spoil(&mut test["evidence"], word);
                if test.get("reason").is_some() {
                    test["reason"] = Value::String(word.to_owned());
                }
            }
        }
    }
    spoiled
}

/// Nothing a person's own programs, files, servers or screens could have said reaches a login part
/// of a published record: with each text of the evidence made in turn a word that could be theirs,
/// and each object given a key and a value of one, the writer publishes none of it. A word that is
/// nothing the writer names, whatever its shape, is not published; only what the build list, the
/// run and the part's own marks fill, and the writer's own fixed texts, are.
#[test]
fn nothing_the_person_could_have_said_survives_in_a_login_part() {
    // Each word, and the part of it that is the person's.
    let words = [
        ("customer.example", "customer.example"),
        ("private-client", "private-client"),
        ("ACME_CORP", "acme_corp"),
        ("someone@example.test", "someone@example.test"),
        ("client-notes.txt", "client-notes"),
        ("~/client/notes", "client/notes"),
        ("/Users/someone/work", "/users/someone"),
        (
            "the agent stopped after customer.example",
            "customer.example",
        ),
        ("nested control/customer.example", "customer.example"),
        (
            "input.write: CUSTOMER_EXAMPLE: no attachment",
            "customer_example",
        ),
    ];
    let mut checked = 0;
    for (path, record) in records(&root()) {
        let entry = entry_of(&record);
        for (word, private) in words {
            let published = written(&spoiled(&record, &entry, word), &entry, "someone");
            for test in login_tests(&published, &entry) {
                checked += 1;
                let text = serde_json::to_string(test)
                    .expect("a test is JSON")
                    .to_lowercase();
                assert!(
                    !text.contains(private),
                    "{}: part {} publishes {private:?} from {word:?}:\n{text}",
                    path.display(),
                    test["part"]
                );
            }
        }
    }
    assert!(
        checked > 0,
        "a record with a login part exists to be checked"
    );
}

/// The reviewer's own cases for the writer: a failure that quotes a word of the person's as a part,
/// a budget, a time or an identifier is published as the code with that word not published; a
/// response's diagnostic is its code; and every time and identifier the record keeps has the shape
/// of one, and names a conversation of the record's own.
#[test]
fn a_failure_is_published_as_its_code_and_a_time_or_identifier_that_is_not_one_is_not() {
    let own = "01a0e444-2331-7621-af42-1113f92f6644";
    let entry = json!({
        "package": "kalareach/example",
        "command": "agent",
        "prefix": "example/1",
        "pinned": "bin/agent",
        "actions": ["prompt.send"],
        "grants": ["upstream.action"],
        "account": {
            "directories": [".agent/*", ".agent/sessions/{date}"],
            "append_only": [".agent/session_index.jsonl"],
            "budget": "example",
            "turns": 50
        }
    });
    let record = json!({
        "schema": "kalareach.conformance/1",
        "run": { "packages": [{ "name": "kalareach/example" }] },
        "identifiers": { "KR-REQ-12.32": { "tests": [{
            "part": "1",
            "outcome": "failed",
            "reason": "the agent stopped after customer.example; the private-client budget has 1 of its 50 turns left",
            "evidence": {
                "device_upload": {
                    "call": "upload.begin", "refused": "INVALID_ARGUMENT", "error": null,
                    "detail": "INVALID_ARGUMENT: upload.begin names customer.example"
                },
                "installed": { "plugin_id": "kalareach/example", "version": "0.1.0", "manifest_digest": "ab", "grant": ["terminal.customer.example", "upstream.action"] },
                "surface": {
                    "capability_records": [
                        { "capability": "agent.customer.example", "usable": false },
                        { "capability": "agent.prompt", "usable": true }
                    ],
                    "answers": [
                        { "call": "agent.prompt.submit", "refused": null, "error": "connection to customer.example failed", "detail": "connection to customer.example failed" },
                        { "call": "customer.example", "refused": "CUSTOMER_EXAMPLE", "error": null, "detail": "CUSTOMER_EXAMPLE: no application instance" }
                    ],
                    "resources": 0,
                    "backend_files": ["registration-customer.example"]
                },
                "reconciled": { "attachment": own, "epoch": 2, "next_sequence": 3, "stale_input": "input.write: AMBIGUOUS_ATTACHMENT: no attachment for customer.example" },
                "person_files": { "appended": [{
                    "file": "~/.agent/session_index.jsonl",
                    "earlier_lines_intact": true,
                    "appended": 2,
                    "others": 0,
                    "own": [
                        { "id": "private-client", "time": "private-client", "holds_the_mark": true, "names_the_run_directory": false, "names_a_conversation_it_created": false },
                        { "id": own, "time": "2026-09-27T19:08:24.432626Z", "holds_the_mark": false, "names_the_run_directory": false, "names_a_conversation_it_created": true }
                    ]
                }] },
                "person_home": {
                    "created_and_removed": [format!("~/.agent/sessions/2026/09/27/rollout-{own}.jsonl"), "~/private-client/notes"]
                },
                "failure_codes": [
                    { "code": "agent_stopped", "after": "customer.example" },
                    { "code": "budget_short", "budget": "private-client", "left": 1, "limit": 50, "need": 2, "part": "1" },
                    { "code": "upload_refused", "refused": "customer.example" },
                    { "code": "customer.example" },
                    { "code": "launch_not_detected", "session": null, "cause": "customer.example", "announced": 1 },
                    { "code": "environment_not_read" },
                    { "code": "agent_stops" },
                    { "code": "part_failed" }
                ]
            }
        }] } }
    });
    let published = written(&record, &entry, "someone");
    let test = &published["identifiers"]["KR-REQ-12.32"]["tests"][0];
    let evidence = &test["evidence"];
    assert_eq!(
        evidence["device_upload"],
        json!({ "call": "upload.begin", "refused": "INVALID_ARGUMENT", "error": null })
    );
    assert_eq!(
        evidence["installed"]["grant"],
        json!(["<not published>", "upstream.action"])
    );
    assert_eq!(
        evidence["surface"],
        json!({
            "capability_records": [
                { "capability": "<not published>", "usable": false },
                { "capability": "agent.prompt", "usable": true }
            ],
            "answers": [
                { "call": "agent.prompt.submit", "refused": null, "error": true },
                { "call": "<not published>", "refused": "<not published>", "error": null }
            ],
            "resources": 0,
            "backend_files": 1
        })
    );
    assert_eq!(
        evidence["reconciled"],
        json!({ "attachment": own, "epoch": 2, "next_sequence": 3, "stale_input": "AMBIGUOUS_ATTACHMENT" })
    );
    assert_eq!(
        evidence["person_files"]["appended"][0]["own"],
        json!([
            { "id": "<not published>", "time": "<not published>", "holds_the_mark": true, "names_the_run_directory": false, "names_a_conversation_it_created": false },
            { "id": own, "time": "2026-09-27T19:08:24.432626Z", "holds_the_mark": false, "names_the_run_directory": false, "names_a_conversation_it_created": true }
        ])
    );
    assert_eq!(
        evidence["person_home"],
        json!({ "created_and_removed": [format!("~/.agent/sessions/2026/09/27/rollout-{own}.jsonl")] })
    );
    assert_eq!(
        evidence["failure_codes"],
        json!([
            { "code": "agent_stopped", "after": "<not published>" },
            { "code": "budget_short", "budget": "<not published>", "left": 1, "limit": 50, "need": 2, "part": "1" },
            { "code": "upload_refused", "refused": "<not published>" },
            { "code": "launch_not_detected", "session": null, "cause": "<not published>", "announced": 1 },
            { "code": "environment_not_read" },
            { "code": "agent_stops" },
            { "code": "part_failed" }
        ])
    );
    assert_eq!(
        test["reason"],
        "the agent stopped after part <not published>; the <not published> budget has 1 of its 50 turns \
         left, fewer than the 2 part 1 can start (the ledger); a paired device's upload.begin is refused on \
         this host (<not published>), so the image is not the device's transfer; the host did not detect \
         the manual launch as section 12 requires: for a cause the part's log states (it announced 1 live \
         instance(s)); the names the session's shell exports could not be read, so that none the build list \
         clears is exported was not established, and the agent was not started or was ended before anything \
         was typed to it; a check of the part failed; \
         its text is in the part's log; the agent stops here: a reason the part's log states"
    );
    assert_eq!(written(&published, &entry, "someone"), published);
}

/// A part of an agent kept apart from the person's own sessions publishes its sandbox, its proxy's
/// hosts and counts, what it left and what the agent's own upkeep rewrote as digests, sizes, times and
/// counts: a path outside the build list's reported paths, a host it was not given, a tool it did not
/// list and a string where a number belongs are not published, and a subagent's start is a stop class
/// of its own.
#[test]
fn a_confined_agent_part_publishes_digests_sizes_and_counts_and_no_name() {
    let digest = "ab".repeat(32);
    let entry = json!({
        "package": "kalareach/example",
        "command": "agent",
        "prefix": "example/1",
        "pinned": "bin/agent",
        "actions": ["prompt.send"],
        "grants": ["upstream.action"],
        "account": {
            "directories": [".agent/*", ".agent/cache"],
            "budget": "example",
            "turns": 50,
            "confinement": {
                "hosts": ["api.example", "auth.example"],
                "unasked_tools": ["Read", "Agent"],
                "reported": ["cache", "logs/agent.log"],
                "residuals": ["a limit of the checks", "another limit"]
            }
        }
    });
    let record = json!({
        "schema": "kalareach.conformance/1",
        "run": { "packages": [{ "name": "kalareach/example" }] },
        "identifiers": { "KR-REQ-12.32": { "tests": [{
            "part": "1",
            "outcome": "failed",
            "evidence": {
                "stop_agent": true,
                "failure_codes": [{ "code": "agent_stops", "class": "subagent_started" }],
                "confinement": {
                    "profile_sha256": digest,
                    "proxy": {
                        "hosts": ["api.example", "customer.example"], "tunnels": 4, "refused": 1,
                        "by_host": [
                            { "host": "api.example", "tunnels": 3, "sent": 100, "received": 200, "open": 0 },
                            { "host": "customer.example", "tunnels": 1, "sent": 1, "received": 1, "open": 0 }
                        ]
                    },
                    "settings": { "rules": 4, "allow_built_in": 0, "mode_manual": true, "loads_more": false, "unlisted": 0, "key": "customer.example" },
                    "subagent_started": false,
                    "new_sessions": 1,
                    "fresh_screen_ms": [133, 55, 54],
                    "resumed_launches": 1,
                    "zero_turn": [{
                        "servers_disabled": 5, "server_list_complete": true,
                        "canary_read_outside_refused": true, "canary_read_inside_works": true,
                        "write_outside_refused": true, "direct_connection_v4_refused": true,
                        "direct_connection_v6_refused": true,
                        "proxy_refused_a_host_it_was_not_given": true,
                        "proxy_tunnelled_to_a_host_it_was_given": true,
                        "person_copy_cannot_run": null, "connections_only_to_the_proxy": true,
                        "connections_seen": 0, "device_keys_reach_the_composer": true,
                        "note": "customer.example"
                    }],
                    "turns": { "charged": 4, "prompts": 3, "steers": 1, "others": 0, "titles": 1, "note": "customer.example" },
                    "unasked_tools": ["Read", "customer-tool"],
                    "residuals": ["a limit of the checks", "customer.example"],
                    "secrets": { "strings": 2, "found_in_run": 0, "found_in_data": 0, "places_in_data": 3, "intermediate": 1, "complete": true },
                    "other_writer_seen": false,
                    "left_in_data": { "trust_records_removed": 1, "trust_records_left": 0, "sessions_bucket_existed": true, "sessions_bucket_left": false, "file_history_removed": 0 }
                },
                "person_home": {
                    "read_whole_after": true,
                    "upkeep": [
                        { "path": "cache", "rewritten": 2, "created": 1, "bytes": 4096, "modified_ms": 1790000000000_i64, "sha256": digest },
                        { "path": "logs/agent.log", "rewritten": 0, "created": 0, "bytes": 0, "modified_ms": null, "sha256": null },
                        { "path": "customer/notes", "rewritten": 1, "created": 0, "bytes": 5, "modified_ms": 1, "sha256": digest },
                        { "path": "cache", "rewritten": "customer.example", "created": 0, "bytes": 0, "modified_ms": 1, "sha256": "customer.example" }
                    ]
                }
            }
        }] } }
    });
    let published = written(&record, &entry, "someone");
    let test = &published["identifiers"]["KR-REQ-12.32"]["tests"][0];
    let evidence = &test["evidence"];
    assert_eq!(
        evidence["confinement"]["proxy"],
        json!({
            "hosts": ["api.example", "<not published>"], "tunnels": 4, "refused": 1,
            "by_host": [
                { "host": "api.example", "tunnels": 3, "sent": 100, "received": 200, "open": 0 },
                { "host": "<not published>", "tunnels": 1, "sent": 1, "received": 1, "open": 0 }
            ]
        })
    );
    assert_eq!(
        evidence["confinement"]["settings"],
        json!({ "rules": 4, "allow_built_in": 0, "mode_manual": true, "loads_more": false, "unlisted": 0 })
    );
    assert_eq!(evidence["confinement"]["subagent_started"], false);
    assert_eq!(evidence["confinement"]["other_writer_seen"], false);
    assert_eq!(evidence["confinement"]["new_sessions"], 1);
    assert_eq!(
        evidence["confinement"]["fresh_screen_ms"],
        json!([133, 55, 54])
    );
    assert_eq!(
        evidence["confinement"]["turns"],
        json!({ "charged": 4, "prompts": 3, "steers": 1, "others": 0, "titles": 1 })
    );
    assert_eq!(evidence["confinement"]["resumed_launches"], 1);
    assert_eq!(
        evidence["confinement"]["zero_turn"][0]["device_keys_reach_the_composer"],
        true
    );
    assert!(
        evidence["confinement"]["zero_turn"][0]
            .get("note")
            .is_none(),
        "a member the writer does not know is not published"
    );
    assert_eq!(
        evidence["confinement"]["residuals"],
        json!(["a limit of the checks", "<not published>"]),
        "only the build list's own statements of what the checks do not cover are published"
    );
    assert_eq!(
        evidence["confinement"]["unasked_tools"],
        json!(["Read", "<not published>"])
    );
    assert_eq!(evidence["confinement"]["profile_sha256"], digest);
    assert_eq!(
        evidence["confinement"]["secrets"],
        json!({ "strings": 2, "found_in_run": 0, "found_in_data": 0, "places_in_data": 3, "intermediate": 1, "complete": true })
    );
    assert_eq!(
        evidence["person_home"]["upkeep"],
        json!([
            { "path": "cache", "rewritten": 2, "created": 1, "bytes": 4096, "modified_ms": 1_790_000_000_000_i64, "sha256": digest },
            { "path": "logs/agent.log", "rewritten": 0, "created": 0, "bytes": 0, "modified_ms": null, "sha256": null },
            { "path": "<not published>", "rewritten": 1, "created": 0, "bytes": 5, "modified_ms": 1, "sha256": digest },
            { "path": "cache", "rewritten": "<not published>", "created": 0, "bytes": 0, "modified_ms": 1 }
        ])
    );
    assert!(
        test["reason"]
            .as_str()
            .is_some_and(|reason| reason.contains("the agent started a subagent")),
        "a subagent's start is said in the writer's words: {}",
        test["reason"]
    );
    assert_eq!(written(&published, &entry, "someone"), published);
}

/// A result that held one of a login's strings is a failure of the part whose record says why the
/// agent stops, in the writer's words and not the driver's text, whatever the part's own outcome
/// was.
#[test]
fn a_part_whose_result_held_a_logins_string_publishes_the_words_of_its_stop() {
    let entry = json!({
        "package": "kalareach/example",
        "command": "agent",
        "prefix": "example/1",
        "pinned": "bin/agent",
        "actions": ["prompt.send"],
        "grants": ["upstream.action"],
        "account": { "budget": "example", "turns": 50 }
    });
    let record = json!({
        "schema": "kalareach.conformance/1",
        "run": { "packages": [{ "name": "kalareach/example" }] },
        "identifiers": { "KR-REQ-12.32": { "tests": [{
            "part": "3",
            "outcome": "failed",
            "reason": "the part's result held a string of the login's files, so it is not kept",
            "evidence": {
                "stop_agent": true,
                "provenance": { "pids": [41, 42] },
                "failure_codes": [{ "code": "agent_stops", "class": "secret_found" }]
            }
        }] } }
    });
    let published = written(&record, &entry, "someone");
    let test = &published["identifiers"]["KR-REQ-12.32"]["tests"][0];
    assert_eq!(test["outcome"], "failed");
    assert!(
        test["reason"].as_str().is_some_and(|reason| reason
            .contains("a string of the login's files was found in what the run wrote")),
        "the stop is said in the writer's words: {}",
        test["reason"]
    );
    assert_eq!(test["evidence"]["stop_agent"], true);
    assert_eq!(written(&published, &entry, "someone"), published);
}

/// The person's servers are published by their identity, and only where the driver put a server's
/// name: the list, the switches that turn them off, and a control that added one. A server that has
/// the name of a word of the build list, of a fixed text or of a placeholder changes none of them,
/// and no placeholder names a server: the writer changes nothing it published, whatever the names.
#[test]
fn servers_are_published_by_their_identity_and_change_no_fixed_text() {
    for names in [
        ["alpha", "alpha.beta"],
        ["alpha.beta", "alpha"],
        ["notes", "server"],
        ["server", "notes"],
        ["mcp", "list"],
        ["skill", "passed"],
        ["enabled", "terminal"],
        ["<server 2>", "alpha"],
        ["alpha", "<server 1>"],
        ["<server 1>customer.example", "alpha"],
    ] {
        let entry = json!({
            "package": "kalareach/example",
            "command": "agent",
            "prefix": "example/1",
            "pinned": "bin/agent",
            "actions": [],
            "account": {
                "switches": ["-c", "a=1"],
                "server_switches": { "switch": ["-c", "mcp_servers.{name}.enabled=false"] },
                "isolated": [
                    { "arguments": ["mcp", "list"], "shows": ["disabled"], "lacks": ["enabled"] },
                    { "arguments": ["exec", "--ephemeral"], "lacks": ["skill", "{servers}"] }
                ]
            }
        });
        let switches: Vec<Value> = ["-c", "a=1"]
            .iter()
            .map(|word| json!(word))
            .chain(names.iter().flat_map(|name| {
                [
                    json!("-c"),
                    json!(format!("mcp_servers.{name}.enabled=false")),
                ]
            }))
            .collect();
        let record = json!({
            "run": { "packages": [{ "name": "kalareach/example" }] },
            "identifiers": { "KR-REQ-12.32": { "tests": [{
                "part": "1",
                "outcome": "passed",
                "evidence": { "tools_offered": [
                    { "probe": ["exec", "--ephemeral"], "check": "passed", "tools": [], "control": { "added": format!("nested control/{}", names[0]), "server": true, "rejected": true } }
                ], "account": { "isolation": {
                    "servers_switched_off": names,
                    "switches": switches,
                    "probes": [
                        { "command": ["mcp", "list"], "shows": ["disabled"], "lacks": ["enabled"] },
                        { "command": ["exec", "--ephemeral"], "lacks": ["skill", "{servers}"] }
                    ],
                    "tools_offered": [
                        { "probe": ["exec", "--ephemeral"], "check": "passed", "tools": ["exec", names[0]], "control": { "added": "nested control/skill", "server": false, "rejected": true } },
                        { "probe": ["exec", "--ephemeral"], "check": "passed", "tools": ["exec"], "control": { "added": format!("nested control/{}", names[0]), "server": true, "rejected": true } }
                    ]
                } } }
            }] } }
        });
        let published = written(&record, &entry, "someone");
        let isolation = &published["identifiers"]["KR-REQ-12.32"]["tests"][0]["evidence"]["account"]
            ["isolation"];
        assert_eq!(
            isolation,
            &json!({
                "servers_switched_off": ["<server 1>", "<server 2>"],
                "switches": ["-c", "a=1", "-c", "mcp_servers.<server 1>.enabled=false", "-c", "mcp_servers.<server 2>.enabled=false"],
                "probes": [
                    { "command": ["mcp", "list"], "shows": ["disabled"], "lacks": ["enabled"] },
                    { "command": ["exec", "--ephemeral"], "lacks": ["skill", "{servers}"] }
                ],
                "tools_offered": [
                    { "probe": ["exec", "--ephemeral"], "control": { "added": "nested control/skill", "server": false, "rejected": true }, "check": "passed", "tools": 2 },
                    { "probe": ["exec", "--ephemeral"], "control": { "server": true, "rejected": true }, "check": "passed", "tools": 1 }
                ]
            }),
            "servers {names:?}"
        );
        assert_eq!(
            published["identifiers"]["KR-REQ-12.32"]["tests"][0]["evidence"]["tools_offered"],
            json!([{ "probe": ["exec", "--ephemeral"], "control": { "server": true, "rejected": true }, "check": "passed", "tools": 0 }]),
            "a part that stopped part way lists its controls at the top, and names no server there either: {names:?}"
        );
        assert_eq!(
            written(&published, &entry, "someone"),
            published,
            "servers {names:?} come out the same the second time"
        );
    }
}

/// A login part that did not run says why as the code the harness gives it, in the words of that
/// code, with the parameters the record can check: a part, the budget the entry names, a count.
#[test]
fn a_login_part_that_did_not_run_says_why_in_the_words_of_its_code() {
    let entry = json!({
        "package": "kalareach/example",
        "command": "agent",
        "prefix": "example/1",
        "pinned": "bin/agent",
        "actions": [],
        "no_account": "the user removed it",
        "account": { "budget": "example", "turns": 50 }
    });
    let test = |part: &str, code: Value| json!({ "part": part, "outcome": "not_run", "reason": "a text with private-client in it", "needs": ["vendor account"], "evidence": { "failure_codes": [code] } });
    let record = json!({
        "run": { "packages": [{ "name": "kalareach/example" }] },
        "identifiers": { "KR-REQ-12.32": { "tests": [
            test("1", json!({ "code": "agent_stopped", "after": "2a" })),
            test("2a", json!({ "code": "budget_short", "budget": "example", "left": 3, "limit": 50, "need": 5, "part": "2a" })),
            test("3", json!({ "code": "no_account" })),
            test("4", json!({ "code": "not_selected" })),
            test("7", json!({ "code": "budget_held", "budget": "private-client" })),
        ] } }
    });
    let published = written(&record, &entry, "someone");
    let reasons: Vec<&str> = published["identifiers"]["KR-REQ-12.32"]["tests"]
        .as_array()
        .expect("tests")
        .iter()
        .map(|test| test["reason"].as_str().expect("a reason"))
        .collect();
    assert_eq!(
        reasons,
        [
            "the agent stopped after part 2a",
            "the example budget has 3 of its 50 turns left, fewer than the 5 part 2a can start (the ledger)",
            "the user removed it",
            "not selected in this run",
            "another run holds the <not published> budget",
        ]
    );
}

/// A part that failed because the agent was not shown to be running its turn when the queued prompt
/// was entered says so in the words of its code, not as a check of the part that failed.
#[test]
fn a_queued_prompt_entered_with_no_turn_shown_running_is_said_by_its_code() {
    let entry = json!({
        "package": "kalareach/example",
        "command": "agent",
        "prefix": "example/1",
        "pinned": "bin/agent",
        "actions": [],
        "account": { "budget": "example", "turns": 50 }
    });
    let record = json!({
        "run": { "packages": [{ "name": "kalareach/example" }] },
        "identifiers": { "KR-REQ-12.32": { "tests": [{
            "part": "2a",
            "outcome": "failed",
            "reason": "a text with private-client in it",
            "evidence": {
                "login_held": true,
                "failure_codes": [{ "code": "queued_prompt_not_during_turn" }]
            }
        }] } }
    });
    let published = written(&record, &entry, "someone");
    let test = &published["identifiers"]["KR-REQ-12.32"]["tests"][0];
    assert_eq!(test["outcome"], "failed");
    assert_eq!(
        test["reason"],
        "the agent was not shown to be running its turn when the second prompt was entered, so the \
         prompt was not shown to wait for a turn that was still running"
    );
    assert_eq!(written(&published, &entry, "someone"), published);
}

/// The variables no agent's session may export whatever its login is: every approved agent's entry
/// clears these, so a person's own keys, tokens and base addresses for any provider, and the cloud
/// accounts an agent can read its model from, reach none of them. An entry adds the names its own
/// agent documents (the home or configuration it moves) and sets the one variable its login is.
const EVERY_ENTRY_CLEARS: [&str; 38] = [
    "OPENAI_*",
    "ANTHROPIC_*",
    "CLAUDE_CODE_*",
    "CODEX_*",
    "GEMINI_*",
    "GOOGLE_*",
    "VERTEX_*",
    "GCLOUD_*",
    "CLOUDSDK_*",
    "CLOUD_ML_*",
    "CODE_ASSIST_*",
    "MOONSHOT_*",
    "KIMI_*",
    "OPENROUTER_*",
    "OPENCODE_*",
    "AWS_*",
    "AZURE_*",
    "CLOUDFLARE_*",
    "*_KEY",
    "*_PAT",
    "*_APIKEY",
    "DATABRICKS_*",
    "INFOMANIAK_*",
    "PRIVATEMODE_*",
    "SNOWFLAKE_*",
    "WATSONX_*",
    "*_ENDPOINT",
    "*_API_KEY",
    "*_API_TOKEN",
    "*_ACCESS_TOKEN",
    "*_AUTH_TOKEN",
    "*_TOKEN",
    "*_SECRET",
    "*_SECRET_KEY",
    "*_ACCESS_KEY",
    "*_PASSWORD",
    "*_BASE_URL",
    "*_API_BASE",
];

/// Whether `pattern` names `name`, as the driver reads a cleared list: a name, a prefix before a
/// closing `*`, or a suffix after an opening `*`; the product's own names (`KR_`) never.
fn names(pattern: &str, name: &str) -> bool {
    if name.starts_with("KR_") {
        return false;
    }
    if let Some(prefix) = pattern.strip_suffix('*') {
        name.starts_with(prefix)
    } else if let Some(suffix) = pattern.strip_prefix('*') {
        name.ends_with(suffix)
    } else {
        pattern == name
    }
}

/// Every entry of the build list that names an account clears every variable of
/// [`EVERY_ENTRY_CLEARS`], in patterns of the three forms the driver reads; the one variable its
/// login is, where it is one, is a name those patterns clear everywhere else.
#[test]
fn every_account_entry_clears_the_variables_that_give_an_agent_another_model_account() {
    let list: Value = serde_json::from_slice(
        &std::fs::read(root().join("fixtures").join("agents").join("builds.json"))
            .expect("the build list reads"),
    )
    .expect("the build list is JSON");
    let mut with_account = 0;
    for entry in list["builds"].as_array().expect("the builds") {
        let package = entry["package"].as_str().expect("a package");
        let Some(account) = entry.get("account").filter(|account| !account.is_null()) else {
            continue;
        };
        with_account += 1;
        let cleared: Vec<&str> = account["cleared"]
            .as_array()
            .unwrap_or_else(|| panic!("{package} lists the variables it clears"))
            .iter()
            .map(|pattern| pattern.as_str().expect("a pattern"))
            .collect();
        for pattern in EVERY_ENTRY_CLEARS {
            assert!(cleared.contains(&pattern), "{package} clears {pattern}");
        }
        for pattern in &cleared {
            let stars = pattern.matches('*').count();
            assert!(
                stars == 0 || (stars == 1 && (pattern.starts_with('*') || pattern.ends_with('*'))),
                "{package}: {pattern} is a name, a prefix or a suffix"
            );
        }
        if let Some(variable) = account.get("variable").and_then(Value::as_str) {
            assert!(
                cleared.iter().any(|pattern| names(pattern, variable)),
                "{package}: its login's variable {variable} is one the entry's patterns clear everywhere else"
            );
        }
    }
    assert!(
        with_account >= 3,
        "the entries with an account: {with_account}"
    );
}

/// An entry whose agent keeps its conversations in a database names the query the driver reads them
/// with: a `SELECT` that writes nothing, whose two columns are a conversation's identifier and a
/// line of JSON, and whose lines carry the members the entry's marks look for, so a mark that
/// the query does not produce is caught here and not by a part that waits for it.
#[test]
fn a_mirror_query_selects_the_lines_the_entrys_marks_look_for() {
    let list: Value = serde_json::from_slice(
        &std::fs::read(root().join("fixtures").join("agents").join("builds.json"))
            .expect("the build list reads"),
    )
    .expect("the build list is JSON");
    let mut mirrored = 0;
    for entry in list["builds"].as_array().expect("the builds") {
        let package = entry["package"].as_str().expect("a package");
        let Some(mirror) = entry["account"]
            .get("mirror")
            .filter(|mirror| !mirror.is_null())
        else {
            continue;
        };
        mirrored += 1;
        let account = &entry["account"];
        let query = mirror["query"].as_str().expect("a query");
        let upper = query.to_uppercase();
        assert!(
            upper.trim_start().starts_with("SELECT SESSION, LINE FROM"),
            "{package}: the query's two columns"
        );
        let words: Vec<&str> = upper
            .split(|character: char| !(character.is_alphanumeric() || character == '_'))
            .collect();
        for word in [
            "INSERT", "UPDATE", "DELETE", "REPLACE", "ATTACH", "PRAGMA", "DROP", "CREATE", "ALTER",
            "VACUUM",
        ] {
            assert!(!words.contains(&word), "{package}: the query holds {word}");
        }
        assert!(!query.contains(';'), "{package}: one statement");
        // Each mark is a run of members of a JSON object as the query's `json_object` writes them:
        // `"role":"user","kind":"text"` is `'role','user','kind','text'` there.
        for key in [
            "prompt_line",
            "reply_line",
            "decision_line",
            "queued_line",
            "turn_line",
        ] {
            let mark = account[key]
                .as_str()
                .unwrap_or_else(|| panic!("{package}: {key}"));
            let pairs: Vec<&str> = mark.split(',').collect();
            let wanted: Vec<String> = pairs
                .iter()
                .map(|pair| {
                    let (name, value) = pair.split_once(':').expect("a member");
                    let name = name.trim_matches('"');
                    match value {
                        // A flag the query computes (`json(CASE ...)`) or states (`json('true')`).
                        "true" => format!("'{name}',json("),
                        text => format!("'{name}','{}'", text.trim_matches('"')),
                    }
                })
                .collect();
            let in_order = wanted.join(",");
            assert!(
                query.contains(&in_order) || wanted.iter().all(|member| query.contains(member)),
                "{package}: the query writes no line the {key} mark {mark} looks for"
            );
        }
        assert_eq!(account["conversations"], "conversation-mirror", "{package}");
        assert!(
            account["interrupt_presses"].as_u64().unwrap_or(1) >= 1,
            "{package}"
        );
    }
    assert_eq!(
        mirrored, 1,
        "the entries that keep their conversations in a database"
    );
}

/// What the harness's sorter of tccd and coreauthd entries makes of a log in each form tccd writes a
/// request in: the older REQUEST, AUTHREQ_CTX and REPLY lines and the REQUEST_MSG and REPLY_MSG
/// entries the system's tccd writes now. A request about the run's programs is a preflight in both,
/// one that is not is told apart, and an entry about them outside any request is marked.
#[test]
fn the_sorter_of_tccd_entries_reads_both_forms_of_a_request() {
    let root = root();
    let script = root.join("scripts").join("e2e-agents.sh");
    let program = std::process::Command::new("bash")
        .arg("-c")
        .arg(r#"eval "$(sed -n "/^tcc_requests=/,/^'$/p" "$0")"; printf '%s' "$tcc_requests""#)
        .arg(&script)
        .output()
        .expect("bash reads the sorter");
    assert!(program.status.success(), "the sorter is in the script");
    let directory = std::env::temp_dir().join(format!("kr-sorter-{}", std::process::id()));
    std::fs::create_dir_all(&directory).expect("a directory");
    std::fs::write(directory.join("sorter.awk"), &program.stdout).expect("the program");
    std::fs::write(directory.join("forms"), "/tmp/krm-1\n").expect("the forms");
    let old = "2026-10-01 14:30:58.694 Df tccd[16382:12ede02a] [com.apple.TCC:access] REQUEST: tccd_uid=501, sender_pid=37624, function=TCCAccessRequest, msgID=37624.1\n\
               2026-10-01 14:30:58.696 Df tccd[16382:12ede02a] [com.apple.TCC:access] AUTHREQ_CTX: msgID=37624.1, function=<private>, service=kTCCServiceDeveloperTool, preflight=yes, query=1,\n\
               2026-10-01 14:30:58.697 Df tccd[16382:12ede02a] [com.apple.TCC:access] AttributionChain: binary_path=/tmp/krm-1/b/kr\n\
               2026-10-01 14:30:58.698 Df tccd[16382:12ede02a] [com.apple.TCC:access] AUTHREQ_RESULT: msgID=37624.1, authValue=0, authReason=5, promptType=1\n\
               2026-10-01 14:30:58.699 Df tccd[16382:12ede02a] [com.apple.TCC:access] REPLY: tccd_uid=501, msgID=37624.1\n";
    let new = |id: &str, preflight: &str, program: &str| {
        format!(
            "2026-10-02 02:28:39.294 I  tccd[5946:13b96d88] [com.apple.TCC:access] REQUEST_MSG: msgID={id}, msg={{\n\
             \trequire_purpose=<xpc_null>\n\tservice=\"kTCCServiceDeveloperTool\"\n\tfunction=\"TCCAccessRequest\"\n\tpreflight={preflight}\n\
             \ttarget_token={{pid:69155, auid:501, euid:501}}\n}}\n\
             2026-10-02 02:28:39.295 I  tccd[5946:13b96d88] [com.apple.TCC:access] AttributionChain: binary_path={program}\n\
             2026-10-02 02:28:39.302 I  tccd[5946:13b96d88] [com.apple.TCC:access] REPLY_MSG: msg={{\n\tprompt_type=1 (0x1)\n\tauth_value=0 (0x0)\n\tresult=false\n}}\n"
        )
    };
    let log = format!(
        "{old}{}{}{}2026-10-02 02:28:40.000 I  tccd[5946:13b96d99] [com.apple.TCC:access] Handling access request from /tmp/krm-1/b/kr-worker\n",
        new("37624.2", "true", "/tmp/krm-1/b/kr-worker"),
        new("37624.3", "false", "/tmp/krm-1/b/kr-hook"),
        new("37624.4", "true", "/usr/bin/other"),
    );
    std::fs::write(directory.join("log"), log).expect("the log");
    let output = std::process::Command::new("awk")
        .arg("-f")
        .arg(directory.join("sorter.awk"))
        .arg(directory.join("forms"))
        .arg(directory.join("log"))
        .output()
        .expect("awk sorts");
    assert!(output.status.success(), "awk sorts the log");
    let mut lines: Vec<String> = String::from_utf8_lossy(&output.stdout)
        .lines()
        .map(|line| line.split(" 2026-").next().unwrap_or_default().to_owned())
        .collect();
    lines.sort();
    assert_eq!(
        lines,
        [
            "outside",
            "request 37624.1 preflight=yes kTCCServiceDeveloperTool authValue=0, authReason=5",
            "request 37624.2 preflight=yes kTCCServiceDeveloperTool authValue=0",
            "request 37624.3 preflight=no kTCCServiceDeveloperTool authValue=0",
        ],
        "a request of either form about the run's programs, the one that is no preflight marked, one about another program left out"
    );
    std::fs::remove_dir_all(&directory).expect("removes the directory");
}

/// Every failure code the harness gives a part it does not run, or fails on the driver's behalf,
/// is one the record's writer knows the words of.
#[test]
fn the_harness_names_only_failure_codes_the_writer_knows() {
    let root = root();
    let output = std::process::Command::new("jq")
        .arg("-L")
        .arg(root.join("scripts"))
        .args(["-nc", r#"include "record-redaction"; known_codes"#])
        .output()
        .expect("jq runs");
    assert!(output.status.success(), "jq lists the writer's codes");
    let known: Vec<String> = serde_json::from_slice(&output.stdout).expect("a list of codes");
    let script = std::fs::read_to_string(root.join("scripts").join("e2e-agents.sh"))
        .expect("the harness reads");
    let mut named = Vec::new();
    for part in script.split("code: \"").skip(1) {
        named.push(part.split('"').next().unwrap_or_default().to_owned());
    }
    for part in script.split("\"code\": \"").skip(1) {
        named.push(part.split('"').next().unwrap_or_default().to_owned());
    }
    assert!(named.len() >= 8, "the harness names its codes: {named:?}");
    for code in &named {
        assert!(
            known.contains(code),
            "the harness names the code {code:?}, which the writer has no words for"
        );
    }
}

/// The driver's outcome line of a part stands, whatever the part's exit status, when it is a
/// failure the part described or a not run the driver names by one of its three codes; the harness
/// asks the record's writer (`driver_line_stands`) and any other not run is replaced.
#[test]
fn the_harness_keeps_the_drivers_failed_and_named_not_run_lines_and_no_other() {
    let lines = [
        json!({ "part": "1", "outcome": "failed", "reason": "x", "evidence": {} }),
        json!({ "part": "1", "outcome": "not_run", "evidence": { "failure_codes": [{ "code": "not_pinned" }] } }),
        json!({ "part": "1", "outcome": "not_run", "evidence": { "failure_codes": [{ "code": "environment_not_clear" }] } }),
        json!({ "part": "1", "outcome": "not_run", "evidence": { "failure_codes": [{ "code": "part_failed" }, { "code": "environment_not_read" }] } }),
        json!({ "part": "1", "outcome": "not_run", "evidence": { "failure_codes": [{ "code": "reply_before_record" }] } }),
        json!({ "part": "1", "outcome": "not_run", "reason": "not the pinned build: x", "evidence": {} }),
        json!({ "part": "1", "outcome": "passed", "evidence": {} }),
        json!({ "part": "2a", "outcome": "failed", "evidence": {} }),
    ];
    let mut child = std::process::Command::new("jq")
        .arg("-L")
        .arg(root().join("scripts"))
        .args([
            "-c",
            "--arg",
            "part",
            "1",
            r#"include "record-redaction"; driver_line_stands($part) | .outcome + " " + ((.evidence.failure_codes // [] | map(.code) | join(",")))"#,
        ])
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .spawn()
        .expect("jq runs");
    let input: String = lines.iter().map(|line| format!("{line}\n")).collect();
    std::io::Write::write_all(child.stdin.as_mut().expect("jq's input"), input.as_bytes())
        .expect("the lines go to jq");
    let output = child.wait_with_output().expect("jq ends");
    assert!(output.status.success(), "jq selects the lines");
    let kept: Vec<String> = String::from_utf8_lossy(&output.stdout)
        .lines()
        .map(|line| line.trim_end_matches('\r').to_owned())
        .collect();
    assert_eq!(
        kept,
        [
            "\"failed \"",
            "\"not_run not_pinned\"",
            "\"not_run environment_not_clear\"",
            "\"not_run part_failed,environment_not_read\""
        ]
    );
}
