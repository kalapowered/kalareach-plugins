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

/// What the shell of a case that runs git starts with: no repository, settings or hooks of the
/// person's, and none of the variables that a hook exports to the program that runs the tests.
const BARE_GIT: &str = "unset GIT_DIR GIT_WORK_TREE GIT_INDEX_FILE GIT_COMMON_DIR \
    GIT_CONFIG_PARAMETERS; export GIT_CONFIG_GLOBAL=/dev/null GIT_CONFIG_NOSYSTEM=1 \
    GIT_CONFIG_COUNT=0;";

/// `command` with the same environment as [`BARE_GIT`] gives a shell.
fn bare_git(command: &mut Command) -> &mut Command {
    for name in [
        "GIT_DIR",
        "GIT_WORK_TREE",
        "GIT_INDEX_FILE",
        "GIT_COMMON_DIR",
        "GIT_CONFIG_PARAMETERS",
    ] {
        command.env_remove(name);
    }
    command
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_CONFIG_COUNT", "0")
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
    // A run that ended with a stop is written as failed, whatever its probes found.
    let mut stopped = result(true, true);
    stopped["outcome"] = json!("failed");
    assert_eq!(
        adopted(stopped),
        json!([]),
        "a failed probe-only run offers nothing"
    );
    let silent = json!({"part": "6b", "outcome": "passed", "evidence": {"shows": {}}});
    assert_eq!(
        adopted(silent),
        json!([]),
        "a run that was no probe-only run adopts nothing"
    );

    let inputs = from_script(
        &[&function("probe_only_inputs")],
        "probe_only_inputs /run/build.json /run/gen /run/probes /run/probe.jsonl /run/scan.jsonl",
    );
    let lines: Vec<&str> = inputs.lines().collect();
    for wanted in [
        "KR_AGENTS_GATEWAY_BUILD=/run/build.json",
        "KR_AGENTS_GENERATION=/run/gen",
        "KR_AGENTS_GATEWAY_PROBE_ONLY=1",
        "KR_AGENTS_GATEWAY_EVIDENCE=/run/probes",
        "KR_AGENTS_RESULT=/run/probe.jsonl",
        "KR_AGENTS_KEY_SCAN=/run/scan.jsonl",
        "KR_AGENTS_SCAN_FD=4",
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

/// A probe-only call is a complete run only where it exited 0 and wrote a `not_run` line for part 6b.
/// One that exited 0 without one, such as a test name that matched nothing, offers no command and
/// says it is not a complete run, and does not read as a run that found nothing to adopt.
#[test]
fn a_probe_only_call_that_exited_0_with_no_not_run_line_is_not_a_complete_run() {
    let says = |exit: u32, outcome: &str| {
        from_script(
            &[
                &function("probe_only_call_is_complete"),
                &function("probe_only_call_says"),
            ],
            &format!("probe_only_call_says 'plugin 1.0' {exit} {outcome} 0 2>&1 || true"),
        )
    };
    let complete = |exit: u32, outcome: &str| {
        from_script(
            &[&function("probe_only_call_is_complete")],
            &format!("probe_only_call_is_complete {exit} {outcome} && echo yes || echo no"),
        )
    };
    assert_eq!(complete(0, "not_run"), "yes");
    assert_eq!(complete(0, "none"), "no", "it exited 0 and wrote no line");
    assert_eq!(complete(0, "passed"), "no", "its line is not not_run");
    assert_eq!(complete(101, "not_run"), "no", "it did not exit 0");
    assert_eq!(
        says(0, "not_run"),
        "plugin 1.0: the newer build's probe-only call exited 0 and offers 0 command(s) to adopt"
    );
    assert_eq!(
        says(0, "none"),
        "plugin 1.0: the newer build's probe-only call exited 0 but wrote no not_run outcome for part 6b (outcome none), so it is not a complete run and offers no command"
    );
    assert_eq!(
        says(101, "failed"),
        "plugin 1.0: the newer build's probe-only call failed (exit 101, outcome failed), so it offers none"
    );
}

/// What the runner's header and usage say of the generation a run installs from is true of every
/// part: part 5b on the stand-in route installs the core repository's own fixture package whatever
/// the generation is, and a candidate lists every pinned build a host can launch, not those with no
/// record.
#[test]
fn the_runners_texts_give_the_stand_in_exception_and_what_a_candidate_lists() {
    let script = std::fs::read_to_string(root().join("scripts").join("e2e-agents.sh"))
        .expect("the runner reads");
    // A comment's lines joined, so a sentence that wraps is read as one.
    let comments = script
        .lines()
        .filter_map(|line| line.strip_prefix('#'))
        .map(str::trim)
        .collect::<Vec<_>>()
        .join(" ");
    let exception = "part 5b on the stand-in route installs the core repository's own fixture package whatever the generation is";
    let (header, usage) = comments
        .split_once("Usage:")
        .expect("the header has a usage");
    assert!(
        header.contains(&format!("(snapshots/development by default; {exception})")),
        "item 4 of the header gives the exception"
    );
    assert!(
        usage.contains(&format!("(default: snapshots/development); {exception};")),
        "the usage of --generation gives the exception"
    );
    assert!(
        script.contains(
            "it names every pinned build a host can launch, without reading a qualification record"
        ),
        "the notice says what a candidate names"
    );
    assert!(
        !script.contains("pinned builds with no record"),
        "a candidate names builds that have a record too"
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

/// A part on a real gateway route reads the values of the other keys this shell holds from a pipe,
/// with no login too: 8b is given one, and the stand-in route and the older driver's parts are not.
#[test]
fn a_part_on_a_real_gateway_route_is_given_the_scan_pipe_login_or_none() {
    let needs = |package: &str, part: &str| {
        from_script(
            &[
                "/^gateway_package=/p",
                &function("part_driver"),
                &function("part_route"),
                &function("needs_the_scan_pipe"),
            ],
            &format!("needs_the_scan_pipe {package} {part} && echo yes || echo no"),
        )
    };
    assert_eq!(needs("kalareach/codex", "8b"), "yes");
    assert_eq!(needs("kalareach/codex", "6b"), "yes");
    assert_eq!(
        needs("kalareach/codex", "5b"),
        "no",
        "the stand-in route reads no pipe"
    );
    assert_eq!(
        needs("kalareach/codex", "2b"),
        "no",
        "the older driver's part has its own"
    );
    assert_eq!(needs("kalareach/claude-code", "8b"), "no");
}

/// A host serves the gateway route only to a build the generation's index lists for the package: the
/// runner tells a person where the generation it installs from lists none.
#[test]
fn the_runner_knows_whether_the_generation_lists_the_pinned_build() {
    let folder = tempfile::tempdir().expect("a temporary folder");
    let targets = folder.path().join("targets");
    std::fs::create_dir_all(&targets).expect("a folder");
    let index = |builds: Value| {
        std::fs::write(
            targets.join("index.json"),
            json!({"entries": [{"plugin_id": "kalareach/codex", "builds": builds}]}).to_string(),
        )
        .expect("the index is written");
    };
    let lists = || {
        from_script(
            &[&function("generation_lists_the_build")],
            &format!(
                "generation_lists_the_build {} kalareach/codex abc && echo yes || echo no",
                folder.path().display()
            ),
        )
    };
    index(json!(null));
    assert_eq!(lists(), "no", "an entry with no builds lists none");
    index(json!([{"executable_digest": "other"}]));
    assert_eq!(lists(), "no");
    index(json!([{"executable_digest": "abc"}]));
    assert_eq!(lists(), "yes");
}

/// The record names the generation a run installed from: plainly where it used the repository's own
/// committed generation with no change to it, and by the signed index's digest, length, number and
/// candidate mark where it used another, such as a candidate, or one that was changed.
#[test]
fn the_record_names_the_generation_the_run_installed_from_in_both_forms() {
    // A generation as the pipeline writes one: the signed targets name the index by its digest, its
    // length and, for a candidate, its mark; the index's generation is a string.
    let write_generation = |folder: &Path, generation: &str, candidate: bool| {
        std::fs::create_dir_all(folder.join("metadata")).expect("a folder");
        std::fs::create_dir_all(folder.join("targets")).expect("a folder");
        std::fs::write(
            folder.join("metadata/targets.json"),
            json!({"signed": {"targets": {"index.json": {
                "hashes": {"sha256": "ab12"}, "length": 345, "custom": {"candidate": candidate}}}}})
            .to_string(),
        )
        .expect("the targets metadata");
        std::fs::write(
            folder.join("targets/index.json"),
            json!({"generation": generation}).to_string(),
        )
        .expect("the index");
    };
    let git = |repository: &Path, arguments: &[&str]| {
        let output = bare_git(&mut Command::new("git"))
            .arg("-C")
            .arg(repository)
            .args(["-c", "user.name=Test", "-c", "user.email=test@example.test"])
            .args([
                "-c",
                "commit.gpgsign=false",
                "-c",
                "core.hooksPath=/dev/null",
            ])
            .args(arguments)
            .output()
            .expect("git runs");
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
    };
    let identity_under = |settings: &str, repository: &Path, source: &Path, copy: &Path| -> Value {
        serde_json::from_str(&from_script(
            &[&function("generation_identity")],
            &format!(
                "{BARE_GIT} {settings} root='{}'; generation_identity '{}' '{}'",
                repository.display(),
                source.display(),
                copy.display()
            ),
        ))
        .expect("the identity is JSON")
    };
    let identity = |repository: &Path, source: &Path, copy: &Path| -> Value {
        identity_under("", repository, source, copy)
    };
    let repository = tempfile::tempdir().expect("a temporary repository");
    let own = repository.path().join("snapshots").join("development");
    write_generation(&own, "6", false);
    git(repository.path(), &["init", "--quiet"]);
    git(repository.path(), &["add", "."]);
    git(
        repository.path(),
        &["commit", "--quiet", "-m", "a committed generation"],
    );
    assert_eq!(
        identity(repository.path(), &own, &own),
        json!("snapshots/development"),
        "the committed generation, with no change to it"
    );

    let candidate = tempfile::tempdir().expect("a temporary folder");
    write_generation(candidate.path(), "7", true);
    let by_index = json!({"sha256": "ab12", "length": 345, "generation": "7", "candidate": true});
    assert_eq!(
        identity(repository.path(), candidate.path(), candidate.path()),
        by_index,
        "a distinct generation is named by what its signed index says"
    );

    // The same folder, changed since it was committed: rebuilt in place, or given a candidate.
    write_generation(&own, "7", true);
    assert_eq!(
        identity(repository.path(), &own, &own),
        by_index,
        "a changed file is named by what its signed index says"
    );
    git(
        repository.path(),
        &["checkout", "--quiet", "--", "snapshots/development"],
    );
    assert_eq!(
        identity(repository.path(), &own, &own),
        json!("snapshots/development"),
        "the change was undone"
    );

    // A run that copied a candidate that stood in the committed folder, and a person who put the
    // committed generation back before the record was written: the run installed from the copy.
    write_generation(&own, "7", true);
    let copies = tempfile::tempdir().expect("a temporary folder");
    let copy = copies.path().join("generation");
    let copied = Command::new("cp")
        .arg("-R")
        .arg(&own)
        .arg(&copy)
        .status()
        .expect("cp runs");
    assert!(copied.success(), "the copy is made");
    git(
        repository.path(),
        &["checkout", "--quiet", "--", "snapshots/development"],
    );
    assert_eq!(
        identity(repository.path(), &own, &own),
        json!("snapshots/development"),
        "the folder is the committed generation again"
    );
    assert_eq!(
        identity(repository.path(), &own, &copy),
        by_index,
        "a copy that is not the committed folder is named by what its signed index says"
    );

    // A file that is not committed is a change, with or without the person's setting that hides
    // such files from `git status`.
    std::fs::write(own.join("targets").join("added.json"), "{}").expect("a file");
    let not_committed =
        json!({"sha256": "ab12", "length": 345, "generation": "6", "candidate": false});
    assert_eq!(
        identity(repository.path(), &own, &own),
        not_committed,
        "a file that is not committed is a change"
    );
    assert_eq!(
        identity_under(
            "export GIT_CONFIG_COUNT=1 GIT_CONFIG_KEY_0=status.showUntrackedFiles \
             GIT_CONFIG_VALUE_0=no;",
            repository.path(),
            &own,
            &own
        ),
        not_committed,
        "a file that is not committed is a change where the person's settings hide it from git"
    );
}

/// The agent stops after a login part that did not establish the login. Both drivers write
/// `stop_agent` where the agent stops, and that is read first. The gateway driver writes no
/// `login_held`, so its part also stops the agent where the results hold no line of it and where it
/// did not pass after a turn was charged, and not where it failed before any charge.
#[test]
fn a_gateway_part_stops_the_agent_where_its_driver_says_so_wrote_no_outcome_or_did_not_pass_after_a_charge()
 {
    let folder = tempfile::tempdir().expect("a temporary folder");
    let results = folder.path().join("results.jsonl");
    let decide = |driver: &str, lines: &[Value], print: &str| {
        let text: Vec<String> = lines.iter().map(Value::to_string).collect();
        std::fs::write(&results, text.join("\n")).expect("the results");
        from_script(
            &[&function("part_line_is"), &function("agent_stops_after")],
            &format!(
                "reason=\"$(agent_stops_after '{}' 6b {driver})\" && {print} || echo no",
                results.display()
            ),
        )
    };
    let stops = |driver: &str, lines: &[Value]| decide(driver, lines, "echo yes");
    let reason = |driver: &str, lines: &[Value]| decide(driver, lines, r#"echo "$reason""#);
    let line = |outcome: &str, charged: u32| json!({"part": "6b", "outcome": outcome, "evidence": {"turns": {"charged": charged}}});
    // The driver's own stop, as the scrub writes its short form: no turns, and the part's name.
    let driver_stop = json!({"part": "6b", "outcome": "failed", "reason": "a key was found", "evidence": {"stop_agent": true}});
    // The scrub's last form names neither the part nor the test.
    let nameless_stop =
        json!({"part": "?", "test": "?", "outcome": "failed", "evidence": {"stop_agent": true}});
    let no_outcome = "part 6b did not pass after its turn was charged, or wrote no outcome, so its login is not shown to hold";

    assert_eq!(
        stops("gateway", &[line("failed", 0)]),
        "no",
        "a failure before any charge"
    );
    assert_eq!(stops("gateway", &[line("failed", 1)]), "yes");
    assert_eq!(stops("gateway", &[line("passed", 1)]), "no");
    assert_eq!(
        stops("gateway", std::slice::from_ref(&driver_stop)),
        "yes",
        "stop_agent with no turns"
    );
    assert_eq!(
        stops("gateway", std::slice::from_ref(&nameless_stop)),
        "yes",
        "only a line that names no part"
    );
    assert_eq!(stops("gateway", &[]), "yes", "no line");
    assert_eq!(reason("gateway", &[line("failed", 1)]), no_outcome);
    assert_eq!(
        reason("gateway", std::slice::from_ref(&nameless_stop)),
        no_outcome
    );
    assert_eq!(
        reason("gateway", std::slice::from_ref(&driver_stop)),
        "part 6b: a key was found"
    );

    let held =
        |held: bool| json!({"part": "6b", "outcome": "failed", "evidence": {"login_held": held}});
    assert_eq!(stops("cases", &[line("passed", 1)]), "no");
    assert_eq!(
        stops("cases", &[held(true)]),
        "no",
        "a failed part whose login holds"
    );
    assert_eq!(stops("cases", &[held(false)]), "yes");
    assert_eq!(stops("cases", &[driver_stop]), "yes");
    assert_eq!(stops("cases", &[nameless_stop]), "yes");
    assert_eq!(stops("cases", &[]), "yes", "no line");
    assert_eq!(
        reason("cases", &[held(false)]),
        "part 6b, which ended without an answer only the agent's model could have given, so its login is not established"
    );
}

/// The recorded command of a part names the pipe it was given: 8b's too, which has no login.
#[test]
fn the_recorded_command_of_a_part_on_a_real_gateway_route_names_its_pipe() {
    let note = |need: u32, part: &str| {
        from_script(
            &[
                "/^gateway_package=/p",
                &function("part_driver"),
                &function("part_route"),
                &function("needs_the_scan_pipe"),
                &function("pipe_note"),
            ],
            &format!("pipe_note {need} kalareach/codex {part}"),
        )
    };
    assert!(
        note(0, "8b").contains("KR_AGENTS_SCAN_FD=4"),
        "8b has a pipe and no login"
    );
    assert!(note(2, "1").contains("KR_AGENTS_SCAN_FD=4"));
    assert_eq!(note(0, "5b"), "", "the stand-in route has none");
}
