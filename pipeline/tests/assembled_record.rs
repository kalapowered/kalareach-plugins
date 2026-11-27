//! A record is assembled by one function, whatever drove the run it describes.
//!
//! `scripts/assemble-record.sh` reads a run's steps, its parts table and the driver's result lines
//! from one JSON file and prints the record in the form `fixtures/agents/README.md` states, as
//! `scripts/e2e-agents.sh` writes it. The inputs in `assembled_record/inputs.json` are small; the
//! cases that hold the record to the form the pipeline checks use the Codex package's own manifest
//! and build list entry.

use std::path::{Path, PathBuf};
use std::process::Output;

use kalareach_catalogue::packages::LoadedPackage;
use kalareach_catalogue::{packages, records, repository_root};
use serde_json::{Value, json};

fn root() -> PathBuf {
    repository_root(Path::new(env!("CARGO_MANIFEST_DIR"))).expect("the repository root is above us")
}

fn fixture(name: &str) -> Value {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("assembled_record")
        .join(name);
    serde_json::from_slice(&std::fs::read(&path).expect("the fixture reads"))
        .expect("the fixture is JSON")
}

/// The assembler run on `inputs`.
fn run_assembler(inputs: &Value) -> Output {
    let directory = tempfile::tempdir().expect("a temporary directory");
    let file = directory.path().join("inputs.json");
    std::fs::write(&file, inputs.to_string()).expect("the inputs are written");
    std::process::Command::new("bash")
        .arg(root().join("scripts").join("assemble-record.sh"))
        .arg(&file)
        .output()
        .expect("bash runs")
}

/// The record the assembler prints for `inputs`.
fn assembled(inputs: &Value) -> Value {
    let output = run_assembler(inputs);
    assert!(
        output.status.success(),
        "the assembler assembles the record: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).expect("the assembler prints JSON")
}

/// The Codex package as this repository publishes it.
fn codex() -> LoadedPackage {
    packages::load(&root())
        .expect("the repository loads")
        .repository
        .packages
        .into_iter()
        .find(|package| package.package.manifest.plugin_id().to_string() == "kalareach/codex")
        .expect("the repository publishes the Codex package")
}

/// The build list's entry for the Codex package, with the ids of its actions under `actions` and the
/// capabilities its manifest requests under `grants`, as the record's writer takes it.
fn writer_entry(inputs: &Value) -> Value {
    let mut entry = inputs["entry"].clone();
    entry["actions"] = inputs["manifest"]["actions"]
        .as_array()
        .into_iter()
        .flatten()
        .map(|action| action["id"].clone())
        .collect();
    entry["grants"] = inputs["manifest"]["capabilities"]
        .as_array()
        .into_iter()
        .flatten()
        .map(|grant| grant["capability"].clone())
        .collect();
    entry
}

/// `record` as the record's writer, `scripts/record-redaction.jq` run with jq, publishes it.
fn written(record: &Value, inputs: &Value) -> Value {
    let mut child = std::process::Command::new("jq")
        .arg("-L")
        .arg(root().join("scripts"))
        .args(["--arg", "user", inputs["user"].as_str().expect("a user")])
        .args(["--argjson", "entry"])
        .arg(writer_entry(inputs).to_string())
        .args(["--argjson", "login_parts"])
        .arg(inputs["login_parts"].to_string())
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

/// The parts of the record in order, as the table names them: each part, the test that is it, and
/// the file that holds the test where it is not the gateway driver's own.
const PARTS: [(&str, &str, &str); 15] = [
    ("1", "a_device_prompts_the_agent", "staged_b.rs"),
    ("2a", "slash_commands_each_work", "staged_c.rs"),
    ("2b", "a_terminal_route_is_advertised", "staged_a.rs"),
    (
        "2c",
        "under_a_binding_an_action_is_permitted",
        "staged_a.rs",
    ),
    ("3", "a_raced_answer_resolves_once", "staged_b.rs"),
    ("4", "a_disconnection_leaves_one_reply", "staged_b.rs"),
    (
        "5a",
        "a_daemon_crash_leaves_the_agent_running",
        "staged_a.rs",
    ),
    ("5b", "a_daemon_killed_leaves_the_agent_running", ""),
    ("6a", "a_running_agent_keeps_its_build", "staged_a.rs"),
    ("6b", "a_process_started_after_an_upgrade", ""),
    ("7", "a_second_process_is_another_execution", "staged_c.rs"),
    (
        "8a",
        "forged_titles_leave_the_host_unchanged",
        "staged_a.rs",
    ),
    ("8b", "a_forged_launch_is_refused", ""),
    ("14.03a", "a_typed_path_reaches_the_composer", "staged_a.rs"),
    ("14.03b", "", ""),
];

/// The parts that ran on the stand-in route.
const STAND_IN: [&str; 2] = ["5a", "5b"];

/// The account and the folders of a run that the record names by placeholder or not at all.
const HOME: &str = "/Users/pat.example";
const SCRATCH: &str = "/private/tmp/krm-ab12cd34";
const TARGET: &str = "/Volumes/Storage/pat-target";
const LOCAL: [&str; 5] = [
    "/Users/",
    "-Users-",
    "pat.example",
    "krm-ab12cd34",
    "pat-target",
];

/// Inputs for the Codex package as this repository publishes it, with a table of every part, a
/// result line for each of `ran`, and a step for each.
fn inputs_for_the_package(ran: &[&str]) -> Value {
    let package = codex();
    let lists: Value = serde_json::from_slice(
        &std::fs::read(root().join("fixtures").join("agents").join("builds.json"))
            .expect("the build list reads"),
    )
    .expect("the build list is JSON");
    let entry = lists["builds"]
        .as_array()
        .into_iter()
        .flatten()
        .find(|build| build["package"] == "kalareach/codex")
        .cloned()
        .expect("the build list pins the Codex package's build");
    let manifest: Value = serde_json::from_slice(
        &std::fs::read(package.directory.join("plugin.json")).expect("the manifest reads"),
    )
    .expect("the manifest is JSON");
    let mut inputs = fixture("inputs.json");
    for (name, value) in [
        ("user", "pat.example".to_owned()),
        ("home", HOME.to_owned()),
        ("evidence", "evidence/run".to_owned()),
        ("evidence_real", format!("{SCRATCH}/evidence")),
        ("temporary", format!("{SCRATCH}/tmp")),
        ("temporary_real", format!("{SCRATCH}/tmp")),
        ("target", TARGET.to_owned()),
        ("target_real", TARGET.to_owned()),
        ("tools", format!("{HOME}/Library/Caches/builds")),
        ("tools_real", format!("{HOME}/Library/Caches/builds")),
        ("shells", format!("{HOME}/Library/Caches/shells")),
    ] {
        inputs[name] = json!(value);
    }
    inputs["entry"] = entry;
    inputs["manifest"] = manifest;
    inputs["manifest_version"] = json!(package.package.manifest.version.to_string());
    inputs["manifest_digest"] = json!(package.manifest_digest.to_string());
    inputs["parts"] = PARTS
        .iter()
        .map(|(part, test, source)| {
            let mut row = json!({
                "part": part, "test": test, "reason": "", "owner": "", "needs": "",
                "driver": "gateway",
                "route": if STAND_IN.contains(part) { "stand_in" } else { "real" },
            });
            if test.is_empty() {
                row["reason"] = json!("no method reads a draft's attachment state");
                row["owner"] = json!("device reads of drafts");
                row["needs"] = json!("draft reads");
            }
            if !source.is_empty() {
                row["source"] = json!(format!("kalareach:tests/e2e/agents/tests/gateway/{source}"));
            }
            row
        })
        .collect();
    let mut steps = vec![inputs["steps"][0].clone()];
    let mut results = Vec::new();
    for (index, part) in ran.iter().enumerate() {
        let test = PARTS
            .iter()
            .find(|row| row.0 == *part)
            .map_or("a_part_the_table_does_not_name", |row| row.1);
        steps.push(json!({
            "number": index + 2, "group": "agents", "what": format!("part {part}"),
            "command": format!("env -i <driver environment> <target>/gateway --exact {test} --nocapture --test-threads=1"),
            "needs": ["KR_SHELL_PACKAGES"], "exit": 0, "seconds": 9, "log": format!("codex/{part}.log"),
            "error": null,
        }));
        results.push(json!({
            "part": part, "test": test, "outcome": "passed", "reason": null,
            "evidence": {
                "resources": 1,
                "route": if STAND_IN.contains(part) { "the application stand-in" } else { "the pinned build" },
                "log": format!("{SCRATCH}/evidence/codex/{part}.log"),
                "ran_in": format!("{SCRATCH}/tmp/{part}"),
                "searched": format!("{HOME}/Library/Caches/builds/codex"),
                "kept": format!("{HOME}/Library/Caches/live/{part}/live.log"),
                "managed": "/Library/Managed Preferences/pat.example/a.plist",
            },
        }));
    }
    inputs["steps"] = json!(steps);
    inputs["results"] = json!(results);
    inputs["selection"] = json!(ran);
    inputs
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

fn tests_of(record: &Value) -> Vec<&Value> {
    records::all_tests(record)
}

fn test_of<'a>(record: &'a Value, part: &str) -> &'a Value {
    tests_of(record)
        .into_iter()
        .find(|test| test["part"] == part)
        .unwrap_or_else(|| panic!("the record has part {part}"))
}

/// An assembled record is in the form every record is held to: its run, its identifiers and their
/// parts, its steps and its summary, its release and the build it ran against. The command of each
/// run of a part is the command of the step it names, in the same words.
#[test]
fn an_assembled_record_is_in_the_form_records_are_held_to() {
    let inputs = inputs_for_the_package(&["2a", "2b", "5a", "5b", "7"]);
    let record = assembled(&inputs);
    let platform = records::Platform::parse("macos-aarch64").expect("a platform");
    assert_eq!(
        records::form_problems(&record, &platform),
        Vec::<String>::new()
    );
    assert_eq!(
        records::release_problems(&record, &codex()),
        Vec::<String>::new()
    );
    assert_eq!(
        records::build_problems(
            &record,
            inputs["entry"]["application"]
                .as_str()
                .expect("an application"),
            inputs["entry"]["version"].as_str().expect("a version"),
            inputs["entry"]["sha256"].as_str().expect("a digest"),
        ),
        Vec::<String>::new()
    );
    let mut compared = 0;
    for test in tests_of(&record) {
        for run in test["runs"].as_array().into_iter().flatten() {
            let step = record["steps"]
                .as_array()
                .expect("steps")
                .iter()
                .find(|step| step["number"] == run["step"])
                .expect("the run names a step of the record");
            assert!(
                step["command"]
                    .as_str()
                    .is_some_and(|text| !text.is_empty())
            );
            assert_eq!(test["command"], step["command"], "part {}", test["part"]);
            compared += 1;
        }
    }
    assert_eq!(compared, 5, "every part that ran names its step");
}

/// A stand-in part with no result line, here the second one (5b), says its route once, by the
/// table's mark, and has no evidence to say it twice: the record is still in the form records are
/// held to.
#[test]
fn a_stand_in_part_with_no_result_line_leaves_the_record_in_the_form_records_are_held_to() {
    let record = assembled(&inputs_for_the_package(&["2b", "5a"]));
    let unselected = test_of(&record, "5b");
    assert_eq!(unselected["outcome"], "not_run");
    assert_eq!(unselected["route"], "stand_in");
    assert!(unselected.get("evidence").is_none());
    let platform = records::Platform::parse("macos-aarch64").expect("a platform");
    assert_eq!(
        records::form_problems(&record, &platform),
        Vec::<String>::new()
    );
}

/// A failed own-home check is named by the number of its own step, wherever the run put it.
#[test]
fn a_failed_own_home_check_is_named_by_its_own_step() {
    let mut inputs = inputs_for_the_package(&["2a", "2b"]);
    let steps = inputs["steps"].as_array_mut().expect("steps");
    let mut own_home = steps.remove(0);
    for (index, step) in steps.iter_mut().enumerate() {
        step["number"] = json!(index + 1);
    }
    own_home["number"] = json!(steps.len() + 1);
    own_home["exit"] = json!(1);
    own_home["error"] = json!("the session named no login keychain");
    let number = own_home["number"].clone();
    steps.push(own_home);
    inputs["own_home"] = json!({});
    inputs["own_home_error"] = json!("the session named no login keychain");
    let record = assembled(&inputs);
    let failures = record["failures_outside_identifiers"]
        .as_array()
        .expect("failures outside the identifiers");
    assert_eq!(failures.len(), 1);
    assert_eq!(failures[0]["step"], number);
    let platform = records::Platform::parse("macos-aarch64").expect("a platform");
    assert_eq!(
        records::form_problems(&record, &platform),
        Vec::<String>::new()
    );
}

/// An assembled record is what the record's writer publishes: the writer, applied to it again,
/// changes nothing, and no text of it names the run's account, its home or the folders it ran in.
#[test]
fn an_assembled_record_is_what_the_record_writer_publishes() {
    let inputs = inputs_for_the_package(&["2a", "2b", "5a", "5b", "7"]);
    let record = assembled(&inputs);
    assert_eq!(written(&record, &inputs), record);
    let mut all = Vec::new();
    strings(&record, &mut all);
    for text in all {
        for local in LOCAL {
            assert!(!text.contains(local), "the record names {local}: {text}");
        }
    }
}

/// A part that ran on the stand-in route says so, one that ran on the real route does not, and the
/// file that holds a part's test is the table's where the table names one.
#[test]
fn a_part_that_ran_on_the_stand_in_route_says_so() {
    let record = assembled(&inputs_for_the_package(&["2b", "5a", "5b"]));
    assert_eq!(test_of(&record, "5a")["route"], "stand_in");
    assert_eq!(test_of(&record, "5b")["route"], "stand_in");
    assert!(test_of(&record, "2b").get("route").is_none());
    assert_eq!(
        test_of(&record, "5a")["source"],
        "kalareach:tests/e2e/agents/tests/gateway/staged_a.rs"
    );
    assert_eq!(
        test_of(&record, "5b")["source"],
        "kalareach:tests/e2e/agents/tests/gateway.rs"
    );
}

/// A part with no result line is not run and says why, and the counts and the verdict follow.
#[test]
fn a_part_with_no_result_line_is_not_run_and_says_why() {
    let record = assembled(&inputs_for_the_package(&["2b"]));
    let skipped = test_of(&record, "6a");
    assert_eq!(skipped["outcome"], "not_run");
    assert_eq!(skipped["reason"], "not selected in this run");
    let cases = &record["identifiers"]["KR-REQ-12.32"];
    assert_eq!(cases["verdict"], "passed");
    assert_eq!(cases["counts"]["passed"], 1);
    assert_eq!(cases["counts"]["not_run"], 12);
    let attachments = &record["identifiers"]["KR-REQ-14.03"];
    assert_eq!(attachments["verdict"], "not_run");
    assert_eq!(attachments["counts"]["not_run"], 2);
}

/// A result line of a part the table does not name is left out: the record is the same with and
/// without it.
#[test]
fn a_result_line_of_a_part_the_table_does_not_name_is_left_out() {
    let without = inputs_for_the_package(&["2b", "7"]);
    let mut with = without.clone();
    with["results"]
        .as_array_mut()
        .expect("results")
        .push(json!({
            "part": "7f", "test": "a_fork_of_a_saved_conversation", "outcome": "failed",
            "reason": "a part the table does not name", "evidence": {"resources": 3},
        }));
    assert_eq!(assembled(&with), assembled(&without));
}

/// The assembler refuses inputs that miss a parameter, and names it.
#[test]
fn the_assembler_refuses_inputs_that_miss_a_parameter() {
    let mut inputs = fixture("inputs.json");
    inputs
        .as_object_mut()
        .expect("inputs are an object")
        .remove("generation_identity");
    let output = run_assembler(&inputs);
    assert_eq!(output.status.code(), Some(2));
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("generation_identity"),
        "the refusal names the parameter: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output.stdout.is_empty(), "a refused run prints no record");
}

/// The harness script assembles its record through the same function: it holds no program of its
/// own for it, and the function makes the record that program made on the same inputs.
#[test]
fn e2e_agents_assembles_through_the_same_function() {
    let script = std::fs::read_to_string(root().join("scripts").join("e2e-agents.sh"))
        .expect("the harness script reads");
    assert!(
        script.contains("assemble_record("),
        "the harness script calls the assembler"
    );
    assert!(
        !script.contains("part_entry("),
        "the harness script keeps no record program of its own"
    );
    assert_eq!(assembled(&fixture("inputs.json")), fixture("expected.json"));
}

// What a gateway part run with the person's login publishes. Every row below is made up: its
// identifiers, its paths under a made-up home, its texts. The driver's fixed words and the build
// list's own values are the real ones, since the redaction keeps a text only where it is one of those.

/// The parts that run with the person's login.
const LOGIN_PARTS: [&str; 6] = ["1", "2a", "3", "4", "6b", "7"];

/// Made-up identifiers of a conversation and of a host's instance.
const CONVERSATION: &str = "0a1b2c3d-1111-4222-8333-444455556666";
const OTHER_CONVERSATION: &str = "ses_aB3dE5gH7jK9mN1pQ3sT";
const INSTANCE: &str = "5e6f7a8b-9999-4aaa-8bbb-ccccddddeeee";
const MARK_END: &str = "KR0123456789AB-END";
const FREE_WORDS: &str = "hello-from-the-persons-programs";

/// The two texts the entry gives the agent's refusal of a second process, made up for the cases.
const REFUSED: [&str; 2] = [
    "this made-up conversation is open in another window",
    "close it there, then try again here",
];

/// The host's words of a launch it did not integrate, which the redaction keeps as a fixed text.
const REFUSAL: &str = "it was not launched through the integration, so no registration names it and none of its bridges is admitted";

/// What the driver says of its two controls in the parts that name them.
const WHAT_PART_2A: &str =
    "orders made wrong on purpose, which the queue and steering checks reject";
const WHY_NOT_PART_2A: &str = "the controls are the agent's own terminal keys, which only the agent could refuse, so the checks are shown wrong orders instead (the guard cases)";
const WHAT_PART_3: &str = "a second approval answered by the local terminal alone";
const WHAT_PART_7: &str = "session B's marker typed into session A";
const WHY_NOT_PART_7: &str =
    "no second execution ran, so there was no second session to type a marker from";
/// Part 2a's control reason as the records of the other packages say it.
const WHY_NOT_PART_2A_BEFORE: &str = "the controls are the agent's own terminal keys, which only the agent could refuse, so the checks are shown wrong orders instead";

/// The state names of the plugin SDK's `CapabilityState`.
const SDK_CAPABILITY_STATES: [&str; 7] = [
    "qualified_available",
    "version_qualified",
    "missing_installation",
    "permission_required",
    "incompatible",
    "temporarily_unavailable",
    "not_tested",
];

/// The words of a value the redaction does not publish.
const NOT_PUBLISHED: &str = "<not published>";

/// The private-looking values the cases offer to the new members. The token is built from pieces
/// so that no pattern of a scanner stands in this file as one literal.
fn private_values() -> Vec<String> {
    vec![
        format!("{HOME}/.codex/diary-2026.md"),
        ["tok", "en-", "9f8e7d6c5b4a39281706f5e4d3c2b1a0"].concat(),
        ["someone", "@", "example.invalid"].concat(),
        CONVERSATION.to_owned(),
        OTHER_CONVERSATION.to_owned(),
        "diary-2026.md".to_owned(),
        FREE_WORDS.to_owned(),
    ]
}

/// `value` with the member at `pointer` replaced by `new`.
fn put(value: &mut Value, pointer: &str, new: Value) {
    *value
        .pointer_mut(pointer)
        .unwrap_or_else(|| panic!("the made-up row has {pointer}")) = new;
}

/// `value` without the member at `pointer`.
fn drop_at(value: &mut Value, pointer: &str) {
    let (parent, key) = pointer.rsplit_once('/').expect("a pointer with a key");
    let parent = value
        .pointer_mut(parent)
        .unwrap_or_else(|| panic!("the made-up row has {pointer}"));
    parent
        .as_object_mut()
        .expect("the parent is an object")
        .remove(key);
}

/// `base` with the members of `extra` added.
fn merged(mut base: Value, extra: Value) -> Value {
    base.as_object_mut()
        .expect("an object")
        .extend(extra.as_object().expect("an object").clone());
    base
}

/// Inputs for one made-up staged result per part of `ran`, with the resume words the cases hold
/// the entry to.
fn staged_inputs(ran: &[&str], poison: impl Fn(&str, &mut Value)) -> Value {
    let mut inputs = inputs_for_the_package(ran);
    inputs["entry"]["account"]["resume"] = json!(["resume", "{conversation}"]);
    inputs["entry"]["account"]["resume_refused"] = json!(REFUSED);
    let account = inputs["entry"]["account"].clone();
    for line in inputs["results"].as_array_mut().expect("results") {
        let part = line["part"].as_str().expect("a part").to_owned();
        let mut evidence = staged(&part, &account);
        poison(&part, &mut evidence);
        line["evidence"] = evidence;
    }
    inputs
}

/// A made-up host instance in a detection, as the driver lists it.
fn instance() -> Value {
    json!({
        "application_instance_id": INSTANCE, "bypass": "not_integrated", "ended_at": null,
        "mode": "native_terminal", "plugin_id": "kalareach/codex",
        "profile_id": format!("lp-1-{INSTANCE}"), "refusal": REFUSAL,
        "started_at": "1760000000000",
    })
}

/// The order a queue or steering check reads, made up.
fn order() -> Value {
    json!({
        "after": {"busy": false, "unfinished": false}, "before": {"busy": true, "unfinished": true},
        "busy_at_submission": true, "first_finished": 1, "new_turn": false, "queued_record": null,
        "records_queue": true, "second_answered": 1, "second_prompt": 1,
    })
}

/// What a part shows, as the staged driver writes it under `shows`, with the build list's own
/// values where the part types them.
fn observations(part: &str, account: &Value) -> Value {
    let common = |turns: u64| json!({"arm": "pinned build", "login": account["login"], "turns_charged": turns});
    match part {
        "1" => merged(
            common(2),
            json!({
                "prompt": {"question": "What is 123 plus 456? Reply with only the number.", "answer": "579"},
                "image": {"syntax": account["image"], "answer": "red", "code": "KR0123456789AB"},
                "images_in_the_record": 1, "published_is_the_sent": true,
                "transfer": {
                    "caller": "paired device", "driver": "kr-client uploads::send",
                    "route": "paired host's data streams",
                    "published": {
                        "byte_len": 321, "content_digest": "ab".repeat(32),
                        "declared_media_type": "image/png", "presented_as_image": true,
                    },
                },
            }),
        ),
        "2a" => json!({
            "arm": "pinned build",
            "driven": merged(common(5), json!({
                "interrupt": {
                    "key": account["interrupt"]["input"], "seen": true,
                    "shows": account["interrupt"]["shows"],
                },
                "slash": {
                    "typed": account["slash"]["input"], "seen": true,
                    "shows": account["slash"]["shows"],
                },
                "queued": {"answer": "579", "order": order()},
                "steering": {"steered": true, "order": order()},
                "control": {"what": WHAT_PART_2A, "breaks_property": false, "why_not": WHY_NOT_PART_2A},
            })),
            "agent_reads": [{
                "call": "agent.capabilities", "error": null, "refused": null,
                "detail": format!("instance {INSTANCE} answered near {HOME}/x"),
            }],
            "surface": {
                "answers": [{
                    "call": "agent.commands", "error": null, "refused": "PERMISSION_DENIED",
                    "detail": format!("instance {INSTANCE} refused near {HOME}/x"),
                }],
                "resources": 3, "backend_files": ["a made-up file", "another"],
                "capability_records": [
                    {"capability": "agent.prompt", "usable": true},
                    {"capability": "agent.steer", "usable": false},
                ],
            },
            "capability_states": [
                ["plugin.metadata.match/1", "qualified_available"],
                ["plugin.approval.respond/1", "not_tested"],
            ],
            "detection": {"waited_ms": 120, "instances": [instance()]},
        }),
        "3" => merged(
            common(2),
            json!({
                "answers_in_the_record": 1, "command_ran": 1,
                "control": {
                    "command_ran": 0, "device_refused": true, "breaks_property": false,
                    "what": WHAT_PART_3, "check": "the control's check, in words",
                },
                "device_answer": "refused: PERMISSION_DENIED: the lease is held",
                "device_won": true,
            }),
        ),
        "4" => merged(
            common(3),
            json!({
                "at_the_drop": {"prompts": 1, "finished_replies": 0},
                "after_reconnect": {
                    "prompts": 1, "finished_replies": 1, "screen_rows_holding_it": 2,
                    "shows": MARK_END,
                },
                "cutoff": {
                    "fresh_screens": 2, "reader_stopped": true, "resyncs": 1,
                    "code_on_a_screen": false, "code_or_reply_mark_in_the_output": false,
                    "event_not_applied": true,
                },
                "control": {
                    "what": "the same prompt sent again", "check": true, "breaks_property": false,
                    "prompts": 2, "finished_replies": 1,
                },
                "markers": {"reply_ends": MARK_END},
                "stale_input": {
                    "refused_with": "AmbiguousAttachment",
                    "detail": format!("attachment {INSTANCE} is not the one"),
                },
            }),
        ),
        "6b" => json!({
            "first_process_image_unchanged": true,
            "fresh_approval_interpreted_by_the_pinned_table": true,
            "second_instance_has_no_gateway": true, "second_process_runs_the_newer_image": true,
            "servers": 2,
        }),
        "7" => merged(
            common(2),
            json!({
                "control": {"what": WHAT_PART_7, "breaks_property": false, "why_not": WHY_NOT_PART_7},
                "conversation": CONVERSATION, "files_before_the_second_start": 4,
                "resume": {"outcome": "refused", "shown": REFUSED[0]},
                "session_a": {"detection": {"waited_ms": 90, "instances": [instance()]}},
                "session_b": {
                    "ended_after_the_refusal": true, "same_conversation": false,
                    "still_running_read_only": true,
                    "started_with": ["resume", CONVERSATION],
                },
            }),
        ),
        other => panic!("part {other} does not run with the login"),
    }
}

/// A made-up staged result: the outer fields of every gateway result, and what the part shows.
fn staged(part: &str, account: &Value) -> Value {
    let mut result = json!({
        "route": "the pinned build", "run_started": true,
        "turns": {
            "charged": 2, "counted_by_occurrence_of_their_line": 0,
            "forms_of_the_names_that_are_no_uuid": [], "in_the_agents_record": 2,
            "named_by_a_name_that_is_no_uuid": 0, "named_by_uuid": 2, "record_files_read": 1,
        },
        "keys": {
            "complete": true, "held_by": [], "held_by_not_listed": 0, "run_gone": true,
            "unread_not_listed": 0, "variable": account["variable"],
        },
        "person_files": {"trees": [{
            "changed": false, "entries_after": 10, "entries_before": 9, "path": "~/.codex",
        }]},
        "moved_work_directory": "none beside the run",
        "shows": observations(part, account),
    });
    if part == "6b" {
        result["newer_turns"] =
            json!({"after_the_probes": 0, "at_the_end": 1, "new_since_the_probes": 1});
    }
    result
}

/// What the record publishes of `staged(part, ..)`: the same, but for the counts that stand for a
/// list, the facts that stand for a text, and what is left out.
fn published(part: &str, account: &Value) -> Value {
    let mut kept = staged(part, account);
    put(
        &mut kept,
        "/turns/forms_of_the_names_that_are_no_uuid",
        json!(0),
    );
    put(&mut kept, "/keys/held_by", json!(0));
    match part {
        "2a" => {
            drop_at(&mut kept, "/shows/agent_reads/0/detail");
            drop_at(&mut kept, "/shows/surface/answers/0/detail");
            put(&mut kept, "/shows/surface/backend_files", json!(2));
            put(&mut kept, "/shows/surface/capability_records", json!(2));
        }
        "3" => {
            put(&mut kept, "/shows/control/check", json!(true));
            put(
                &mut kept,
                "/shows/device_answer",
                json!("PERMISSION_DENIED"),
            );
        }
        "4" => {
            drop_at(&mut kept, "/shows/stale_input/detail");
            put(
                &mut kept,
                "/shows/stale_input/refused_with",
                json!("AMBIGUOUS_ATTACHMENT"),
            );
        }
        "7" => {
            drop_at(&mut kept, "/shows/conversation");
            put(
                &mut kept,
                "/shows/session_b/started_with",
                json!(["resume", "{conversation}"]),
            );
        }
        _ => {}
    }
    kept
}

/// The record assembled from a made-up staged result, and the record's writer applied to the
/// record again changes nothing.
fn kept_record(part: &str, inputs: &Value) -> Value {
    let record = assembled(inputs);
    assert_eq!(
        written(&record, inputs),
        record,
        "the writer changes the record of part {part}"
    );
    record
}

/// The part's evidence in the record assembled from its made-up staged result.
fn kept_evidence(part: &str, inputs: &Value) -> Value {
    test_of(&kept_record(part, inputs), part)["evidence"].clone()
}

/// The evidence a part run with the login keeps is every member the driver fills from the build
/// list, the run and the part's own marks, and counts, with none of its free text.
fn a_login_part_keeps_what_its_row_observed(part: &str) {
    let inputs = staged_inputs(&[part], |_, _| {});
    let account = inputs["entry"]["account"].clone();
    assert_eq!(
        kept_evidence(part, &inputs),
        published(part, &account),
        "part {part}"
    );
}

#[test]
fn part_1_keeps_what_its_row_observed() {
    a_login_part_keeps_what_its_row_observed("1");
}

#[test]
fn part_2a_keeps_what_its_row_observed() {
    a_login_part_keeps_what_its_row_observed("2a");
}

#[test]
fn part_3_keeps_what_its_row_observed() {
    a_login_part_keeps_what_its_row_observed("3");
}

#[test]
fn part_4_keeps_what_its_row_observed() {
    a_login_part_keeps_what_its_row_observed("4");
}

#[test]
fn part_6b_keeps_what_its_row_observed() {
    a_login_part_keeps_what_its_row_observed("6b");
}

#[test]
fn part_7_keeps_what_its_row_observed() {
    a_login_part_keeps_what_its_row_observed("7");
}

/// Private-looking values in the new members, by part: where each is put, and what the record
/// publishes there instead. A member that is left out has `None`.
fn poisons(part: &str) -> Vec<(&'static str, Value, Option<Value>)> {
    let values = private_values();
    let (home_file, token, address) = (values[0].clone(), values[1].clone(), values[2].clone());
    let words = || json!(FREE_WORDS);
    let np = || Some(json!(NOT_PUBLISHED));
    let mut all = vec![
        (
            "/moved_work_directory",
            json!(format!("{home_file} {token}")),
            np(),
        ),
        ("/keys/variable", json!(token), np()),
        (
            "/turns/forms_of_the_names_that_are_no_uuid",
            json!([CONVERSATION, "diary-2026.md"]),
            Some(json!(2)),
        ),
    ];
    let own: Vec<(&'static str, Value, Option<Value>)> = match part {
        "1" => vec![
            ("/shows/arm", json!(address), np()),
            ("/shows/login", json!(token), np()),
            ("/shows/prompt/question", words(), np()),
            ("/shows/prompt/answer", words(), np()),
            ("/shows/image/syntax", words(), np()),
            ("/shows/transfer/caller", json!(home_file), np()),
            (
                "/shows/transfer/published/declared_media_type",
                json!(address),
                np(),
            ),
            (
                "/shows/transfer/published/content_digest",
                json!(token),
                np(),
            ),
        ],
        "2a" => vec![
            ("/shows/driven/slash/typed", words(), np()),
            ("/shows/driven/control/what", words(), np()),
            ("/shows/driven/control/why_not", words(), np()),
            ("/shows/driven/queued/answer", words(), np()),
            (
                "/shows/agent_reads/0/call",
                json!(format!("agent.prompt {token}")),
                np(),
            ),
            ("/shows/surface/answers/0/refused", json!(address), np()),
            (
                "/shows/capability_states/0",
                json!([token, "qualified_available"]),
                np(),
            ),
            ("/shows/detection/instances/0/plugin_id", json!(token), np()),
        ],
        "3" => vec![
            (
                "/shows/device_answer",
                json!(format!("PERMISSION_DENIED: {home_file} {token} {address}")),
                Some(json!("PERMISSION_DENIED")),
            ),
            ("/shows/control/what", words(), np()),
            ("/shows/control/check", words(), Some(json!(true))),
        ],
        "4" => vec![
            ("/shows/stale_input/refused_with", words(), np()),
            ("/shows/markers/reply_ends", json!(token), np()),
            ("/shows/after_reconnect/shows", words(), np()),
            ("/shows/control/what", words(), np()),
        ],
        "6b" => vec![
            ("/shows/servers", words(), np()),
            ("/shows/first_process_image_unchanged", words(), np()),
        ],
        "7" => vec![
            ("/shows/login", json!(address), np()),
            ("/shows/control/why_not", words(), np()),
            ("/shows/resume/shown", words(), np()),
            ("/shows/resume/outcome", words(), np()),
            (
                "/shows/session_b/started_with",
                json!([
                    "resume",
                    CONVERSATION,
                    OTHER_CONVERSATION,
                    FREE_WORDS,
                    token
                ]),
                Some(json!([
                    "resume",
                    "{conversation}",
                    "{conversation}",
                    NOT_PUBLISHED,
                    NOT_PUBLISHED
                ])),
            ),
        ],
        other => panic!("part {other} does not run with the login"),
    };
    all.extend(own);
    all
}

/// A value of the person's, or one that looks like it, in a member the record now keeps for a part
/// run with the login, reaches no text of the record: the member reads `<not published>`, or is
/// the code or the count the value stands for.
#[test]
fn private_looking_values_in_the_new_members_do_not_reach_the_record() {
    for part in LOGIN_PARTS {
        let poisons = poisons(part);
        let inputs = staged_inputs(&[part], |_, evidence| {
            for (pointer, value, _) in &poisons {
                put(evidence, pointer, value.clone());
            }
        });
        let record = assembled(&inputs);
        assert_eq!(written(&record, &inputs), record, "part {part}");
        let kept = &test_of(&record, part)["evidence"];
        for (pointer, _, published) in &poisons {
            assert_eq!(
                kept.pointer(pointer),
                published.as_ref(),
                "part {part} at {pointer}"
            );
        }
        let private = private_values();
        let mut all = Vec::new();
        strings(&record, &mut all);
        for text in all {
            for private in private.iter().map(String::as_str).chain(LOCAL) {
                assert!(
                    !text.contains(private),
                    "part {part}: the record names {private}: {text}"
                );
            }
        }
    }
}

/// The directories a part's tree lists are the watched ones, in the shape five other parts
/// publish: the directory, the changed flag and the two counts, and no name of an entry. A tree
/// outside them, and any other member of a tree, is not published.
#[test]
fn a_tree_outside_the_watched_directories_or_naming_an_entry_is_not_published() {
    let watched =
        json!({"path": "~/.codex", "changed": true, "entries_before": 4, "entries_after": 5});
    for part in LOGIN_PARTS {
        let trees = json!([
            {"path": "~/Documents", "changed": true, "entries_before": 1, "entries_after": 2},
            {
                "path": "~/.codex", "changed": true, "entries_before": 4, "entries_after": 5,
                "names": ["diary-2026.md"], "entries": [{"name": "diary-2026.md"}],
            },
            {"path": "~/.codex", "changed": "yes", "entries_before": 4, "entries_after": 5},
            {"path": "~/.codex", "changed": false, "entries_before": "4", "entries_after": 5},
            {"path": "~/.codex/sessions", "changed": false, "entries_before": 1, "entries_after": 1},
        ]);
        let inputs = staged_inputs(&[part], |_, evidence| {
            put(evidence, "/person_files/trees", trees.clone());
        });
        let record = assembled(&inputs);
        assert_eq!(written(&record, &inputs), record, "part {part}");
        assert_eq!(
            test_of(&record, part)["evidence"]["person_files"],
            json!({"trees": [watched]}),
            "part {part}"
        );
        let mut all = Vec::new();
        strings(&record, &mut all);
        for text in all {
            for outside in ["~/Documents", "diary-2026.md", "~/.codex/sessions"] {
                assert!(!text.contains(outside), "part {part}: {outside} in {text}");
            }
        }
    }
}

/// A part's `person_files` is left out where nothing of it is published: no tree in a watched
/// directory, no tree at all, or an empty member. Members of it that the schema keeps stay.
#[test]
fn person_files_is_left_out_when_nothing_of_it_is_published() {
    let outside =
        json!([{"path": "~/Documents", "changed": true, "entries_before": 1, "entries_after": 2}]);
    for part in LOGIN_PARTS {
        for person_files in [
            json!({"trees": outside}),
            json!({"trees": []}),
            json!({"trees": "~/.codex"}),
            json!({}),
        ] {
            let inputs = staged_inputs(&[part], |_, evidence| {
                put(evidence, "/person_files", person_files.clone());
            });
            let kept = kept_evidence(part, &inputs);
            assert!(
                kept.get("person_files").is_none(),
                "part {part} with {person_files}: {kept}"
            );
        }
        let inputs = staged_inputs(&[part], |_, evidence| {
            drop_at(evidence, "/person_files");
        });
        assert!(kept_evidence(part, &inputs).get("person_files").is_none());
        let inputs = staged_inputs(&[part], |_, evidence| {
            put(
                evidence,
                "/person_files",
                json!({"keychain_item_rewritten": false, "trees": outside}),
            );
        });
        assert_eq!(
            kept_evidence(part, &inputs)["person_files"],
            json!({"keychain_item_rewritten": false}),
            "part {part}"
        );
    }
}

/// A part's `shows` is left out where none of it is published: a part whose observations are
/// free words, or an empty member, publishes no `shows`.
#[test]
fn shows_is_left_out_when_nothing_of_it_is_published() {
    for part in LOGIN_PARTS {
        for shows in [json!({"text": FREE_WORDS}), json!({})] {
            let inputs = staged_inputs(&[part], |_, evidence| {
                put(evidence, "/shows", shows.clone());
            });
            let kept = kept_evidence(part, &inputs);
            assert!(
                kept.get("shows").is_none(),
                "part {part} with {shows}: {kept}"
            );
            assert_eq!(kept["route"], "the pinned build", "part {part}");
        }
    }
}

/// Part 7 publishes the agent's refusal only in the entry's own words, and no identifier of a
/// conversation: the second start's words keep the entry's words and name the conversation by its
/// placeholder.
#[test]
fn part_7_publishes_the_entrys_refusal_words_and_no_conversation_identifier() {
    for (shown, kept) in [
        (REFUSED[0], REFUSED[0]),
        (REFUSED[1], REFUSED[1]),
        ("this is some other made-up text", NOT_PUBLISHED),
    ] {
        let inputs = staged_inputs(&["7"], |_, evidence| {
            put(evidence, "/shows/resume/shown", json!(shown));
        });
        let evidence = kept_evidence("7", &inputs);
        assert_eq!(evidence["shows"]["resume"]["shown"], kept);
        assert_eq!(evidence["shows"]["resume"]["outcome"], "refused");
        assert_eq!(
            evidence["shows"]["session_b"]["started_with"],
            json!(["resume", "{conversation}"])
        );
        assert_eq!(evidence["shows"]["session_b"]["same_conversation"], false);
        assert!(evidence["shows"].get("conversation").is_none());
    }
}

/// A device's answer is published as the one error code its text holds, and as nothing where it
/// holds none or more than one.
#[test]
fn a_device_answer_is_published_as_the_one_code_it_holds() {
    for (answer, kept) in [
        (
            "refused: PERMISSION_DENIED: the lease is held",
            "PERMISSION_DENIED",
        ),
        ("PERMISSION_DENIED", "PERMISSION_DENIED"),
        ("PERMISSION_DENIED and then SESSION_CLOSED", NOT_PUBLISHED),
        ("a refusal that names no code", NOT_PUBLISHED),
        ("XPERMISSION_DENIEDX", NOT_PUBLISHED),
    ] {
        let inputs = staged_inputs(&["3"], |_, evidence| {
            put(evidence, "/shows/device_answer", json!(answer));
        });
        assert_eq!(
            kept_evidence("3", &inputs)["shows"]["device_answer"],
            kept,
            "{answer}"
        );
    }
}

/// The record assembled from a made-up row of `part`, with the member at each pointer replaced.
fn record_with(part: &str, replaced: &[(&str, Value)]) -> Value {
    let inputs = staged_inputs(&[part], |_, evidence| {
        for (pointer, value) in replaced {
            put(evidence, pointer, value.clone());
        }
    });
    kept_record(part, &inputs)
}

/// No text of `record` names a value of the person's, or any of `values`.
fn assert_names_none_of(record: &Value, values: &[&str], context: &str) {
    let mut all = Vec::new();
    strings(record, &mut all);
    for text in all {
        for value in values.iter().copied().chain(LOCAL) {
            assert!(
                !text.contains(value),
                "{context}: the record names {value}: {text}"
            );
        }
    }
}

/// Part 4 publishes the error its stale input was refused with as the protocol's own code, never
/// as the name the driver wrote, and as nothing where that name is not the name of a code. The
/// code stands as it is, since the writer applies the redaction to a published record again.
#[test]
fn part_4_publishes_the_protocols_code_of_the_refusal_and_not_the_name_written() {
    let with_path = format!("AmbiguousAttachment {HOME}/.codex/diary-2026.md");
    for (written, kept) in [
        ("AmbiguousAttachment", "AMBIGUOUS_ATTACHMENT"),
        ("PermissionDenied", "PERMISSION_DENIED"),
        ("IdConflict", "ID_CONFLICT"),
        ("NotInKrSession", "NOT_IN_KR_SESSION"),
        ("AMBIGUOUS_ATTACHMENT", "AMBIGUOUS_ATTACHMENT"),
        ("AMBIGUOUS_ATTACHMENT AMBIGUOUS_SESSION", NOT_PUBLISHED),
        ("Ambiguous_Attachment", NOT_PUBLISHED),
        ("ambiguousattachment", NOT_PUBLISHED),
        ("AmbiguousAttachments", NOT_PUBLISHED),
        ("AmbiguousAttachment AmbiguousSession", NOT_PUBLISHED),
        ("NoSuchCode", NOT_PUBLISHED),
        ("", NOT_PUBLISHED),
        (with_path.as_str(), NOT_PUBLISHED),
    ] {
        let record = record_with("4", &[("/shows/stale_input/refused_with", json!(written))]);
        assert_eq!(
            test_of(&record, "4")["evidence"]["shows"]["stale_input"],
            json!({"refused_with": kept}),
            "{written}"
        );
        let names: &[&str] = if written.is_empty() || written == kept {
            &[]
        } else {
            &[written]
        };
        assert_names_none_of(&record, names, written);
    }
}

/// A moved work directory that names a path is not published: the one verdict the driver writes
/// when nothing moved stands, and any other text of the member reads `<not published>`.
#[test]
fn a_moved_work_directory_that_names_a_path_is_not_published() {
    let rows = [
        format!("{HOME}/Documents/moved-work-1"),
        format!("{SCRATCH}/moved-work-2"),
        "~/Documents/moved-work-3".to_owned(),
        format!("none beside the run: {HOME}/Documents/moved-work-4"),
        format!("moved-work-5 {HOME}/.codex/diary-2026.md"),
    ];
    for part in LOGIN_PARTS {
        for row in &rows {
            let record = record_with(part, &[("/moved_work_directory", json!(row))]);
            assert_eq!(
                test_of(&record, part)["evidence"]["moved_work_directory"],
                NOT_PUBLISHED,
                "part {part}: {row}"
            );
            let context = format!("part {part}: {row}");
            assert_names_none_of(&record, &["moved-work-", "diary-2026.md"], &context);
        }
        let record = record_with(part, &[]);
        assert_eq!(
            test_of(&record, part)["evidence"]["moved_work_directory"],
            "none beside the run",
            "part {part}"
        );
    }
}

/// A device's answer that holds a path and a code is published as the code alone, and one that
/// holds a path and no code or two codes as nothing.
#[test]
fn a_device_answer_that_holds_a_path_and_a_code_publishes_the_code_and_not_the_path() {
    for (answer, kept) in [
        (
            format!("refused: PERMISSION_DENIED: no lease for {HOME}/.codex/diary-2026.md"),
            "PERMISSION_DENIED",
        ),
        (
            format!("{HOME}/.codex/diary-2026.md: PERMISSION_DENIED"),
            "PERMISSION_DENIED",
        ),
        (
            format!("PERMISSION_DENIED {SCRATCH}/device-answer-3 {HOME}/Documents/device-answer-4"),
            "PERMISSION_DENIED",
        ),
        (
            format!("refused near {HOME}/.codex/diary-2026.md"),
            NOT_PUBLISHED,
        ),
        (
            format!("{HOME}/.codex/diary-2026.md: PERMISSION_DENIED, SESSION_CLOSED"),
            NOT_PUBLISHED,
        ),
    ] {
        let record = record_with("3", &[("/shows/device_answer", json!(answer))]);
        assert_eq!(
            test_of(&record, "3")["evidence"]["shows"]["device_answer"],
            kept,
            "{answer}"
        );
        assert_names_none_of(&record, &["diary-2026.md", "device-answer-"], &answer);
    }
}

/// A `resume.shown` is published only as one of the entry's own refusal texts: the same text with
/// words before or after it, two of the texts joined, or a part of one reads `<not published>`.
#[test]
fn a_resume_shown_longer_than_the_refusal_text_is_not_published() {
    let first = REFUSED[0];
    let rows = [
        format!("{first} {HOME}/.codex/diary-2026.md"),
        format!("{first} {}", REFUSED[1]),
        format!("{first}."),
        format!("{}\n", REFUSED[1]),
        format!("note: {}", REFUSED[1]),
        format!("{first} {CONVERSATION}"),
        first[..first.len() - 1].to_owned(),
    ];
    for shown in &rows {
        let record = record_with("7", &[("/shows/resume/shown", json!(shown))]);
        let resume = &test_of(&record, "7")["evidence"]["shows"]["resume"];
        assert_eq!(resume["shown"], NOT_PUBLISHED, "{shown}");
        assert_eq!(resume["outcome"], "refused", "{shown}");
        assert_names_none_of(&record, &["diary-2026.md", CONVERSATION], shown);
    }
}

/// A pair of `capability_states` is published only where its first member is the form of a
/// capability the manifest grants and its second is one of the SDK's state names; any other text
/// of a state, or a pair of another shape, reads `<not published>` in the pair's place.
#[test]
fn a_capability_pair_with_a_free_text_state_is_not_published() {
    let id = "plugin.metadata.match/1";
    let states: Vec<Value> = SDK_CAPABILITY_STATES
        .iter()
        .map(|state| json!([id, state]))
        .collect();
    let record = record_with("2a", &[("/shows/capability_states", json!(states))]);
    assert_eq!(
        test_of(&record, "2a")["evidence"]["shows"]["capability_states"],
        json!(states)
    );
    let free_states = [
        FREE_WORDS.to_owned(),
        format!("qualified_available {HOME}/.codex/diary-2026.md"),
        "Qualified_Available".to_owned(),
        "qualified_available ".to_owned(),
        "available".to_owned(),
        String::new(),
    ];
    for state in &free_states {
        let pairs = json!([[id, state], [id, "not_tested"]]);
        let record = record_with("2a", &[("/shows/capability_states", pairs)]);
        assert_eq!(
            test_of(&record, "2a")["evidence"]["shows"]["capability_states"],
            json!([NOT_PUBLISHED, [id, "not_tested"]]),
            "{state:?}"
        );
        assert_names_none_of(&record, &[FREE_WORDS, "diary-2026.md"], state);
    }
    let other_shapes = [
        json!([id]),
        json!([id, "not_tested", FREE_WORDS]),
        json!([id, 7]),
        json!([id, ["not_tested"]]),
        json!([FREE_WORDS, "not_tested"]),
        json!(FREE_WORDS),
    ];
    for pair in other_shapes {
        let record = record_with("2a", &[("/shows/capability_states", json!([pair]))]);
        assert_eq!(
            test_of(&record, "2a")["evidence"]["shows"]["capability_states"],
            json!([NOT_PUBLISHED]),
            "{pair}"
        );
        assert_names_none_of(&record, &[FREE_WORDS], &pair.to_string());
    }
}

/// The words a second start was given are published as the entry's words and the placeholder for
/// a conversation; a word beyond them, or two words in one, reads `<not published>`.
#[test]
fn a_started_with_with_one_more_word_publishes_the_extra_word_as_not_published() {
    let path = format!("{HOME}/.codex/diary-2026.md");
    let np = NOT_PUBLISHED;
    for (row, kept) in [
        (
            json!(["resume", CONVERSATION, path]),
            json!(["resume", "{conversation}", np]),
        ),
        (
            json!(["resume", CONVERSATION, "--last"]),
            json!(["resume", "{conversation}", np]),
        ),
        (
            json!(["resume", CONVERSATION, FREE_WORDS]),
            json!(["resume", "{conversation}", np]),
        ),
        (
            json!(["resume", format!("resume {CONVERSATION}")]),
            json!(["resume", np]),
        ),
        (json!([format!("resume {CONVERSATION}")]), json!([np])),
        (json!(format!("resume {CONVERSATION}")), json!(np)),
    ] {
        let record = record_with("7", &[("/shows/session_b/started_with", row.clone())]);
        assert_eq!(
            test_of(&record, "7")["evidence"]["shows"]["session_b"]["started_with"],
            kept,
            "{row}"
        );
        let names = [CONVERSATION, "diary-2026.md", FREE_WORDS, "--last"];
        assert_names_none_of(&record, &names, &row.to_string());
    }
}

/// Both texts of part 2a's control reason stand as written: the one the records of the other
/// packages hold, and the one the Codex part says now. A text that differs from them does not.
#[test]
fn both_texts_of_part_2as_control_reason_stand_in_their_set() {
    for why_not in [WHY_NOT_PART_2A_BEFORE, WHY_NOT_PART_2A] {
        let record = record_with("2a", &[("/shows/driven/control/why_not", json!(why_not))]);
        assert_eq!(
            test_of(&record, "2a")["evidence"]["shows"]["driven"]["control"]["why_not"],
            why_not
        );
    }
    for other in [
        format!("{WHY_NOT_PART_2A} {HOME}/.codex/diary-2026.md"),
        WHY_NOT_PART_2A.replace("wrong orders", "right orders"),
        format!("{WHY_NOT_PART_2A_BEFORE} (the cases)"),
    ] {
        let record = record_with("2a", &[("/shows/driven/control/why_not", json!(other))]);
        assert_eq!(
            test_of(&record, "2a")["evidence"]["shows"]["driven"]["control"]["why_not"],
            NOT_PUBLISHED,
            "{other}"
        );
        assert_names_none_of(
            &record,
            &["diary-2026.md", "right orders", "(the cases)"],
            &other,
        );
    }
}
