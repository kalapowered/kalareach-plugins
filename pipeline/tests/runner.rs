//! The qualification runner: which driver runs each part, what the record says of the route and the
//! driver, what the build it hands a driver holds, and what it adopts from a probe-only run.
//!
//! `scripts/e2e-agents.sh` runs a real host and a real agent, so none of it runs here. What is
//! read here is what it is made of: its parts table and the functions it reads out of, which are
//! taken from the script as it is written, and the record writer's jq functions.

use std::path::{Path, PathBuf};
use std::process::Command;

use kalareach_catalogue::repository_root;
use serde_json::{Value, json};

const GATEWAY_FILE: &str = "kalareach:tests/e2e/agents/tests/gateway.rs";
const CASES_FILE: &str = "kalareach:tests/e2e/agents/tests/cases.rs";

fn root() -> PathBuf {
    repository_root(Path::new(env!("CARGO_MANIFEST_DIR"))).expect("the repository root is above us")
}

/// `program` run by jq with the record writer's functions in scope, each of `arguments` a JSON
/// variable of its name.
fn jq(program: &str, arguments: &[(&str, Value)]) -> Value {
    let mut command = Command::new("jq");
    command.arg("-n").arg("-L").arg(root().join("scripts"));
    for (name, value) in arguments {
        command.arg("--argjson").arg(name).arg(value.to_string());
    }
    command.arg(format!(r#"include "record-redaction"; {program}"#));
    let output = command.output().expect("jq runs");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).expect("jq writes JSON")
}

/// What `shell` prints after the script's own `definitions` are read out of it. Each definition is
/// a line range of the script, as `sed -n` takes it.
fn from_script(definitions: &[&str], shell: &str) -> String {
    let script = root().join("scripts").join("e2e-agents.sh");
    let mut program = String::new();
    for range in definitions {
        program.push_str(&format!(r#"eval "$(sed -n '{range}' "$0")"; "#));
    }
    program.push_str(shell);
    let output = Command::new("bash")
        .arg("-c")
        .arg(program)
        .arg(&script)
        .output()
        .expect("bash runs");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8_lossy(&output.stdout)
        .trim_end()
        .to_owned()
}

/// The script's function `name`, as a range for [`from_script`].
fn function(name: &str) -> String {
    format!("/^{name}() {{/,/^}}/p")
}

/// The parts table of the script, one row to a line.
fn parts_table() -> Vec<String> {
    from_script(&["/^parts=(/,/^)/p"], r#"printf '%s\n' "${parts[@]}""#)
        .lines()
        .map(str::to_owned)
        .collect()
}

/// The part a table row is of, and its test, which is empty for a part that waits.
fn row(row: &str) -> (&str, &str) {
    let mut fields = row.split('|');
    (
        fields.next().unwrap_or_default(),
        fields.next().unwrap_or_default(),
    )
}

/// A part the gateway driver runs is recorded as its test, its file and its step, and a part the
/// older driver runs as before.
#[test]
fn a_part_run_through_the_gateway_driver_names_the_gateway_file_and_its_own_test_target() {
    let result = json!({"part": "6b", "outcome": "passed", "evidence": {"shows": {}}});
    let gateway = json!({"part": "6b", "test": "gateway_a_test", "reason": "", "owner": "",
        "needs": "", "driver": "gateway"});
    let older = json!({"part": "6b", "test": "an_older_test", "reason": "", "owner": "",
        "needs": "", "driver": "cases"});
    let entry = |row: &Value, result: &Value| {
        jq(
            "part_entry($row; $result; {\"6b\": 2}; {\"6b\": \"run it\"}; {}; [])",
            &[("row", row.clone()), ("result", result.clone())],
        )
    };
    let ran = entry(&gateway, &result);
    assert_eq!(ran["source"], GATEWAY_FILE);
    assert_eq!(ran["test"], "kr-e2e-agents --test gateway gateway_a_test");
    assert_eq!(ran["runs"], json!([{"step": 2, "outcome": "passed"}]));
    assert_eq!(ran["command"], "run it");
    let skipped = entry(&gateway, &Value::Null);
    assert_eq!(skipped["source"], GATEWAY_FILE);
    assert_eq!(skipped["outcome"], "not_run");
    let before = entry(&older, &result);
    assert_eq!(before["source"], CASES_FILE);
    assert_eq!(before["test"], "kr-e2e-agents --test cases an_older_test");
}

/// A part's driver is the one that holds its test: the gateway driver's tests are named
/// `gateway_...`, and no other driver's are.
#[test]
fn every_part_that_names_a_test_runs_through_the_driver_that_holds_that_test() {
    let mut gateway = Vec::new();
    for table_row in parts_table() {
        let (part, test) = row(&table_row);
        if test.is_empty() {
            continue;
        }
        let driver = from_script(
            &["/^gateway_package=/p", &function("part_driver")],
            &format!("part_driver kalareach/codex {part}"),
        );
        assert_eq!(
            driver == "gateway",
            test.starts_with("gateway_"),
            "part {part}: {test} is run through the {driver} driver"
        );
        if driver == "gateway" {
            gateway.push(part.to_owned());
        }
    }
    gateway.sort();
    assert_eq!(gateway, ["5b", "6b", "8b"]);
}

/// The parts that run with the login are the parts that charge a turn: the script's list of them,
/// which the record writer reads, is the parts whose turn count is above none, and the writer's
/// own copy in the cases that check it names the same.
#[test]
fn the_login_lists_name_the_parts_that_charge_a_turn_and_no_other() {
    let turns = |part: &str| -> u32 {
        from_script(
            &["/^gateway_package=/p", &function("login_turns")],
            &format!("login_turns kalareach/codex {part}"),
        )
        .parse()
        .expect("a number of turns")
    };
    let mut charged: Vec<String> = parts_table()
        .iter()
        .map(|table_row| row(table_row).0.to_owned())
        .filter(|part| turns(part) > 0)
        .collect();
    charged.sort();
    assert_eq!(charged, ["1", "2a", "3", "4", "6b", "7"]);
    assert_eq!(
        turns("6b"),
        1,
        "6b starts one turn, which the ledger reserves"
    );
    assert_eq!(
        turns("5b"),
        0,
        "5b runs on the stand-in route, which has no login"
    );
    assert_eq!(turns("8b"), 0, "8b is run with no login");
    let list_of = |package: &str| -> Value {
        serde_json::from_str(&from_script(
            &["/^gateway_package=/p", &function("login_parts_for")],
            &format!("login_parts_for {package}"),
        ))
        .expect("the script's list is JSON")
    };
    let list = list_of("kalareach/codex");
    assert_eq!(list, json!(charged));
    // Another package's 6b is not run, so it is no login part there.
    assert_eq!(
        list_of("kalareach/claude-code"),
        json!(["1", "2a", "3", "4", "7"])
    );
    let writers = include_str!("agents.rs");
    let expected = format!(
        "const LOGIN_PARTS: &str = r#\"{}\"#;",
        list.to_string().replace(',', ", ")
    );
    let others = format!(
        "const LOGIN_PARTS_OTHER: &str = r#\"{}\"#;",
        list_of("kalareach/claude-code")
            .to_string()
            .replace(',', ", ")
    );
    assert!(
        writers.contains(&others),
        "the record writer's cases hold the other packages' list: {others}"
    );
    assert!(
        writers.contains(&expected),
        "the record writer's cases hold a copy of the list: {expected}"
    );
}

/// 5b has no real arm: the real route refuses it, so the runner gives its driver no build, runs the
/// stand-in route, and the record says so. A part with a real arm is given the build, by the
/// variable its driver reads.
#[test]
fn a_part_that_has_no_real_arm_is_given_no_build_and_is_recorded_as_the_stand_in_route() {
    let route = |part: &str| {
        from_script(
            &["/^gateway_package=/p", &function("part_route")],
            &format!("part_route kalareach/codex {part}"),
        )
    };
    assert_eq!(route("5b"), "stand_in");
    for part in ["1", "2a", "6b", "8b"] {
        assert_eq!(route(part), "real", "part {part}");
    }
    let given = |part: &str| {
        from_script(
            &[
                "/^gateway_package=/p",
                &function("part_driver"),
                &function("part_route"),
                &function("build_variable"),
            ],
            &format!("build_variable kalareach/codex {part} /run/build.json"),
        )
    };
    assert_eq!(given("5b"), "", "the stand-in route is given no build");
    assert_eq!(given("6b"), "KR_AGENTS_GATEWAY_BUILD=/run/build.json");
    assert_eq!(given("8b"), "KR_AGENTS_GATEWAY_BUILD=/run/build.json");
    assert_eq!(given("1"), "KR_AGENTS_BUILD=/run/build.json");

    let row = json!({"part": "5b", "test": "gateway_a_test", "reason": "", "owner": "",
        "needs": "", "driver": "gateway", "route": "stand_in"});
    let result = json!({"part": "5b", "outcome": "passed", "evidence": {}});
    let marked = jq(
        "part_entry($row; $result; {}; {}; {}; [])",
        &[("row", row.clone()), ("result", result.clone())],
    );
    assert_eq!(marked["route"], "stand_in");
    let skipped = jq("part_entry($row; null; {}; {}; {}; [])", &[("row", row)]);
    assert_eq!(skipped["route"], "stand_in");
    let real = json!({"part": "6b", "test": "gateway_a_test", "reason": "", "owner": "",
        "needs": "", "driver": "gateway", "route": "real"});
    let entry = jq(
        "part_entry($row; $result; {}; {}; {}; [])",
        &[("row", real), ("result", result)],
    );
    assert!(entry.get("route").is_none(), "a real run carries no mark");
}

/// The build the driver reads carries the newer build, with the commands that say it loads nothing
/// of the person's own where the build list gives them; a newer build that is not installed is none.
#[test]
fn the_build_a_driver_reads_carries_the_newer_build_and_its_isolation_probes() {
    let entry = json!({
        "package": "kalareach/codex", "application": "codex", "version": "1.0.0",
        "prefix": "codex/1.0.0", "pinned": "bin/codex", "sha256": "ab", "command": "codex",
        "launch": "native", "arguments": [], "runtime": [], "environment": {}, "home": {},
        "ready": "Welcome", "harmless": {}, "composer": {}, "server": null, "transcript": "t",
        "account": null,
        "newer": {"version": "1.1.0", "prefix": "codex/1.1.0", "pinned": "bin/codex",
            "sha256": "cd", "ready": "Welcome",
            "isolated": [{"arguments": ["--version"], "shows": "1.1.0"}]}
    });
    let built = |installed: bool| {
        jq(
            "$entry | driver_build(\"/tools\"; \"/repo\"; {}; []; $installed)",
            &[("entry", entry.clone()), ("installed", json!(installed))],
        )
    };
    let newer = &built(true)["newer"];
    assert_eq!(newer["version"], "1.1.0");
    assert_eq!(newer["prefix"], "/tools/codex/1.1.0");
    assert_eq!(
        newer["isolated"],
        json!([{"arguments": ["--version"], "shows": "1.1.0"}])
    );
    assert_eq!(built(false)["newer"], Value::Null);
    let mut bare = entry.clone();
    bare["newer"]["isolated"] = Value::Null;
    let none = jq(
        "$entry | driver_build(\"/tools\"; \"/repo\"; {}; []; true)",
        &[("entry", bare)],
    );
    assert_eq!(
        none["newer"]["isolated"],
        json!([]),
        "a newer build with none has none"
    );
}

/// The newer build's isolation probes are adopted from a probe-only run only where the run's record
/// allows adoption, and then only those the record shows adoptable; the call that makes the run
/// asks for it by the probe-only variable and names the evidence folder the runner made.
#[test]
fn the_newer_builds_probes_are_adopted_only_from_a_probe_only_record_that_allows_it() {
    let entry = json!({"account": {"isolated": [
        {"arguments": ["mcp", "list"], "lacks": ["server"]},
        {"arguments": ["features", "list"], "shows": "off"}]}});
    let result = |allowed: bool, second_adoptable: bool| {
        json!({"part": "6b", "outcome": "not_run", "evidence": {"shows": {
            "probe_only": true,
            "adoption": {"allowed": allowed, "refused_because": []},
            "probes": [
                {"arguments": ["mcp", "list"], "adoptable": true},
                {"arguments": ["features", "list"], "adoptable": second_adoptable}]}}})
    };
    let adopted = |result: Value| {
        jq(
            "$result | adopted_isolation($entry)",
            &[("result", result), ("entry", entry.clone())],
        )
    };
    assert_eq!(
        adopted(result(true, true)),
        entry["account"]["isolated"],
        "a record that allows adoption gives the probes it ran"
    );
    assert_eq!(
        adopted(result(false, false)),
        json!([]),
        "no adoption where it is refused"
    );
    let silent = json!({"part": "6b", "outcome": "passed", "evidence": {"shows": {}}});
    assert_eq!(
        adopted(silent),
        json!([]),
        "a run that was no probe-only run adopts nothing"
    );

    let inputs = from_script(
        &[&function("probe_only_inputs")],
        "probe_only_inputs /run/build.json /run/gen /run/probes /run/probe.jsonl",
    );
    let lines: Vec<&str> = inputs.lines().collect();
    for wanted in [
        "KR_AGENTS_GATEWAY_BUILD=/run/build.json",
        "KR_AGENTS_GENERATION=/run/gen",
        "KR_AGENTS_GATEWAY_PROBE_ONLY=1",
        "KR_AGENTS_GATEWAY_EVIDENCE=/run/probes",
        "KR_AGENTS_RESULT=/run/probe.jsonl",
    ] {
        assert!(lines.contains(&wanted), "{wanted} is in {lines:?}");
    }
    assert!(
        lines
            .iter()
            .all(|line| !line.starts_with("KR_AGENTS_TURNS")),
        "a probe-only run has no ledger and starts no turn"
    );
}

/// The package a run installs is the generation's, so the generation the runner copies holds the
/// package the record is for: the manifest digest of the tree's package is the one its index names.
#[test]
fn the_generation_a_run_installs_from_holds_the_package_the_record_is_for() {
    let generation = root().join("snapshots").join("development");
    let manifest = root().join("plugins/kalareach/codex/plugin.json");
    let check = |digest: &str| {
        Command::new("bash")
            .arg("-c")
            .arg(format!(
                r#"eval "$(sed -n '{}' "$0")"; generation_holds_the_package "$1" kalareach/codex "$2""#,
                function("generation_holds_the_package")
            ))
            .arg(root().join("scripts/e2e-agents.sh"))
            .arg(&generation)
            .arg(digest)
            .output()
            .expect("bash runs")
    };
    let digest: String =
        <sha2::Sha256 as sha2::Digest>::digest(std::fs::read(&manifest).expect("the manifest"))
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect();
    assert!(
        check(&digest).status.success(),
        "the tree's package is the generation's"
    );
    let other = check(&"0".repeat(64));
    assert!(!other.status.success(), "another digest is not");
    assert!(
        String::from_utf8_lossy(&other.stderr).contains("names another package"),
        "{}",
        String::from_utf8_lossy(&other.stderr)
    );
}

/// A part the table records as not run has no driver log: the record writer leaves it as the table
/// wrote it when its part becomes one that runs with the login, and still rewrites the reason of a
/// part the driver ran.
#[test]
fn a_part_the_table_records_as_not_run_is_not_rewritten_as_a_login_part_that_stopped() {
    let table = json!({"test": "part 6b", "part": "6b",
        "source": "kalareach-plugins:fixtures/agents/README.md", "keyed_by": "case_table",
        "outcome": "not_run", "reason": "waits for a landing (the owner)", "needs": ["landing"]});
    let driver = json!({"test": "kr-e2e-agents --test gateway a_test", "part": "6b",
        "source": GATEWAY_FILE, "keyed_by": "attached_comment", "outcome": "not_run",
        "reason": "whatever the driver said", "evidence": {"failure_codes": [{"code": "not_selected"}]}});
    let record = json!({"identifiers": {"KR-REQ-12.32": {"tests": [table.clone(), driver]}}});
    let written = jq(
        r#"$record | redact_record("someone"; {account: {}}; ["6b"])"#,
        &[("record", record)],
    );
    let tests = &written["identifiers"]["KR-REQ-12.32"]["tests"];
    assert_eq!(tests[0], table, "the table's entry is as it was written");
    assert_eq!(tests[1]["reason"], "not selected in this run");
}

/// The gateway driver installs and guards the Codex package alone: another package's parts 5b, 6b
/// and 8b wait with their earlier reason, run through the older driver's path, and are no login
/// part or build variable of the gateway.
#[test]
fn the_gateway_parts_of_another_package_wait_as_they_did_before_the_driver_had_them() {
    let rows = |package: &str| -> Vec<String> {
        from_script(
            &[
                "/^parts=(/,/^)/p",
                "/^gateway_package=/p",
                "/^waiting_rows=(/,/^)/p",
                &function("agent_parts"),
            ],
            &format!("agent_parts {package}"),
        )
        .lines()
        .map(str::to_owned)
        .collect()
    };
    let codex = rows("kalareach/codex");
    let other = rows("kalareach/claude-code");
    assert_eq!(
        codex.len(),
        other.len(),
        "the same parts, in the same order"
    );
    for part in ["5b", "6b", "8b"] {
        let find = |rows: &[String]| {
            rows.iter()
                .find(|table_row| row(table_row).0 == part)
                .cloned()
                .expect("the part is in the table")
        };
        assert!(
            row(&find(&codex)).1.starts_with("gateway_"),
            "part {part} of Codex"
        );
        let waiting = find(&other);
        assert_eq!(
            row(&waiting).1,
            "",
            "part {part} of another package has no test"
        );
        assert!(
            waiting.contains("waits for the command integration"),
            "part {part} of another package says why it waits: {waiting}"
        );
    }
    let calls = from_script(
        &[
            "/^gateway_package=/p",
            &function("part_driver"),
            &function("part_route"),
            &function("build_variable"),
            &function("login_turns"),
        ],
        r#"printf '%s %s %s %s\n' "$(part_driver kalareach/claude-code 6b)" "$(part_route kalareach/claude-code 5b)" "$(build_variable kalareach/claude-code 8b /b.json)" "$(login_turns kalareach/claude-code 6b)""#,
    );
    assert_eq!(calls, "cases real KR_AGENTS_BUILD=/b.json 0");
}

/// The checks after a part read the process numbers and the installed package that only the older
/// driver writes; the gateway driver's parts are not held to fields it does not write.
#[test]
fn the_checks_after_a_part_read_only_what_its_driver_writes() {
    let writes = |package: &str, part: &str| {
        from_script(
            &[
                "/^gateway_package=/p",
                &function("part_driver"),
                &function("writes_installed_and_pids"),
            ],
            &format!("writes_installed_and_pids {package} {part} && echo yes || echo no"),
        )
    };
    for part in ["5b", "6b", "8b"] {
        assert_eq!(
            writes("kalareach/codex", part),
            "no",
            "the gateway driver's part {part}"
        );
    }
    assert_eq!(writes("kalareach/codex", "2b"), "yes");
    assert_eq!(
        writes("kalareach/claude-code", "6b"),
        "yes",
        "6b of another package is the older driver's"
    );
}
