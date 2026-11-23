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

/// A part the staged driver wrote for a login keeps the evidence it states in fixed words and
/// numbers, and none of its free text is published. The evidence keys of a staged result that the
/// schema does not keep are printed.
#[test]
fn a_staged_login_part_keeps_the_fixed_evidence_of_a_gateway_login_part() {
    let mut inputs = inputs_for_the_package(&["2a", "7"]);
    let staged = [
        (
            "2a",
            json!({
                "route": "the pinned build", "run_started": true,
                "turns": {"charged": 3, "in_the_agents_record": 3},
                "arm": "pinned build", "login": "a login", "turns_charged": 3,
                "slash": {"seen": true}, "interrupt": {"seen": true},
                "control": {"breaks_property": false},
                "shows": {"text": "something the person's own programs said"},
            }),
        ),
        (
            "7",
            json!({
                "route": "the pinned build", "run_started": true,
                "turns": {"charged": 2},
                "arm": "pinned build", "login": "a login", "files_before_the_second_start": [],
                "session_a": {"detection": {}}, "session_b": {"same_conversation": false},
                "resume": {"outcome": "refused"},
                "shows": {"text": "something the person's own programs said"},
            }),
        ),
    ];
    for line in inputs["results"].as_array_mut().expect("results") {
        if let Some((_, evidence)) = staged.iter().find(|(part, _)| line["part"] == *part) {
            line["evidence"] = evidence.clone();
        }
    }
    let record = assembled(&inputs);
    for (part, evidence) in &staged {
        let kept = &test_of(&record, part)["evidence"];
        assert_eq!(kept["route"], "the pinned build", "part {part}");
        assert_eq!(kept["run_started"], true, "part {part}");
        assert!(kept["turns"]["charged"].is_u64(), "part {part}");
        assert!(
            kept.get("shows").is_none(),
            "free text is not published in part {part}: {kept}"
        );
        let dropped: Vec<&str> = evidence
            .as_object()
            .expect("evidence is an object")
            .keys()
            .filter(|key| kept.get(key.as_str()).is_none())
            .map(String::as_str)
            .collect();
        println!("part {part}: evidence keys the schema does not keep: {dropped:?}");
    }
}
