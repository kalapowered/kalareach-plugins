//! Every package in the repository validates, and a broken one does not.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use kalareach_catalogue::{fixtures, packages, repository_root};
use kr_plugin_sdk::connector::{FieldPath, FieldSegment, MethodClass, ResponseCorrelation};
use kr_plugin_sdk::matching::MatchConfidence;

fn root() -> PathBuf {
    repository_root(Path::new(env!("CARGO_MANIFEST_DIR"))).expect("the repository root is above us")
}

#[test]
fn every_package_in_the_repository_validates() {
    let loaded = packages::load(&root()).expect("the repository loads");
    assert!(
        loaded.rejected.is_empty(),
        "{:?}",
        loaded
            .rejected
            .iter()
            .map(|rejected| (&rejected.relative, &rejected.report.findings))
            .collect::<Vec<_>>()
    );
    assert!(!loaded.repository.packages.is_empty());
    assert!(!loaded.repository.publishers.is_empty());
}

#[test]
fn every_package_belongs_to_a_publisher_with_a_record() {
    let loaded = packages::load(&root()).expect("the repository loads");
    for package in &loaded.repository.packages {
        let publisher = package.package.manifest.publisher_id.to_string();
        assert!(
            loaded.repository.publishers.contains_key(&publisher),
            "{publisher} has no record"
        );
    }
}

#[test]
fn every_package_fixture_matches_its_own_predicates() {
    let loaded = packages::load(&root()).expect("the repository loads");
    let mut cases = 0;
    for package in &loaded.repository.packages {
        cases += fixtures::check(&package.relative, &package.directory, &package.package)
            .unwrap_or_else(|error| panic!("{}: {error}", package.relative));
    }
    assert!(cases > 0, "no package carries a fixture");
}

#[test]
fn the_bundled_connectors_claim_only_what_they_carry() {
    let loaded = packages::load(&root()).expect("the repository loads");
    for package in &loaded.repository.packages {
        let manifest = &package.package.manifest;
        if package.package.connector.is_some() {
            continue;
        }
        // Without a declarative native-proxy table there is nothing qualified for the gateway to
        // interpret, so a package that ships no connector claims no upstream or approval trust.
        for request in &manifest.capabilities {
            assert!(
                request.capability.within_default_ceiling(),
                "{} requests {} without a connector table",
                package.relative,
                request.capability
            );
        }
        for action in &manifest.actions {
            assert!(
                !action.effect.is_mutation(),
                "{} declares the mutation {} without a connector table",
                package.relative,
                action.effect
            );
        }
    }
}

/// The bundled connectors, and the identities each one is qualified against.
///
/// A generic check cannot tell whether two packages swapped their applications, because both
/// arrangements are structurally sound. The repository knows its own packages, so the expected
/// pairing is written here and a new connector has to be added deliberately.
const BUNDLED_CONNECTORS: &[BundledConnector] = &[
    BundledConnector {
        plugin: "kalareach/claude-code",
        protocol: "claude-code-channels",
        registry: "npm",
        identifier: "@anthropic-ai/claude-code",
        file_stem: "claude",
        frames: &[
            (
                "a message delivered into the session",
                "notifications/claude/channel",
            ),
            ("a method the table does not list", "tools/call"),
            (
                "a relayed tool approval",
                "notifications/claude/channel/permission_request",
            ),
            (
                "an answer naming a request nobody issued",
                "notifications/claude/channel/permission",
            ),
            (
                "the answer to that approval",
                "notifications/claude/channel/permission",
            ),
        ],
        required_members: &[
            (
                "notifications/claude/channel/permission_request",
                "params.tool_name",
            ),
            ("notifications/claude/channel/permission", "params.behavior"),
        ],
    },
    BundledConnector {
        plugin: "kalareach/codex",
        protocol: "codex-app-server",
        registry: "npm",
        identifier: "@openai/codex",
        file_stem: "codex",
        frames: &[
            ("a method the table does not list", "thread/name/set"),
            ("an uncertain turn/start result", "turn/start"),
            ("an uncertain turn/start result", "turn/started"),
            (
                "competing approvals",
                "item/commandExecution/requestApproval",
            ),
            ("competing approvals", "item/fileChange/requestApproval"),
            (
                "methods this table refuses",
                "account/chatgptAuthTokens/refresh",
            ),
            ("methods this table refuses", "thread/shellCommand"),
            ("reconnect", "initialize"),
            ("reconnect", "turn/interrupt"),
            ("serverRequest/resolved", "serverRequest/resolved"),
            ("thread subscriptions", "item/started"),
            ("thread subscriptions", "thread/start"),
            ("thread subscriptions", "thread/started"),
            ("thread subscriptions", "thread/unsubscribe"),
            ("turn/steer and its expectedTurnId", "turn/steer"),
        ],
        required_members: &[
            (
                "item/commandExecution/requestApproval",
                "params.startedAtMs",
            ),
            ("item/fileChange/requestApproval", "params.startedAtMs"),
            ("item/started", "params.turnId"),
            ("serverRequest/resolved", "params.requestId"),
            ("turn/interrupt", "params.turnId"),
            ("turn/steer", "params.expectedTurnId"),
        ],
    },
];

struct BundledConnector {
    plugin: &'static str,
    protocol: &'static str,
    registry: &'static str,
    identifier: &'static str,
    file_stem: &'static str,
    /// Every frame the corpus has to carry, as a situation and a wire name, in sorted order.
    frames: &'static [(&'static str, &'static str)],
    /// Members a frame has to carry whatever the corpus says about itself, by wire name and
    /// dotted path. A corpus that drops both a member and its own expectation of it would
    /// otherwise stop checking the thing its situation is named after.
    required_members: &'static [(&'static str, &'static str)],
}

#[test]
fn a_native_bridge_grant_states_where_the_code_runs() {
    let loaded = packages::load(&root()).expect("the repository loads");
    let mut recipes = 0;
    for package in &loaded.repository.packages {
        let Some(bridge) = &package.package.manifest.native_bridge.0 else {
            continue;
        };
        recipes += 1;
        // The validator checks that a recipe is complete and removable. It cannot check that the
        // grant a person reads before accepting it says what accepting it means. Section 11 asks
        // for two facts, and both have to be asserted rather than mentioned: "runs inside the
        // sandbox with its own permissions; nothing runs outside the sandbox" contains every word
        // and states the opposite. So the clause is read in order, from "runs under" onwards.
        // The disclosure is pinned as one clause rather than a set of words in an order. "runs
        // under Claude Code's own permissions inside the sandbox; it never runs outside
        // Wasmtime's sandbox" uses every word and discloses the opposite.
        let statement = bridge.grant_statement.as_str();
        assert!(
            statement.contains("runs under"),
            "{}: the grant does not say what the bridge runs under: {statement}",
            package.relative
        );
        assert!(
            statement.contains("own permissions"),
            "{}: the grant does not say whose permissions it runs under: {statement}",
            package.relative
        );
        assert!(
            statement.contains("outside the KalaReach plugin sandbox, outside Wasmtime"),
            "{}: the grant does not disclose that the bridge runs outside the plugin sandbox and \
             outside Wasmtime, which section 11 requires it to: {statement}",
            package.relative
        );
        assert!(
            statement.contains("Removal"),
            "{}: the grant does not say what removal takes back out: {statement}",
            package.relative
        );
    }
    assert!(recipes > 0, "no package ships a native bridge");
}

#[test]
fn a_connector_names_one_protocol_and_identifies_its_application_exactly() {
    let loaded = packages::load(&root()).expect("the repository loads");
    let expected: BTreeMap<&str, &BundledConnector> = BUNDLED_CONNECTORS
        .iter()
        .map(|entry| (entry.plugin, entry))
        .collect();
    let mut seen = BTreeSet::new();
    for package in &loaded.repository.packages {
        let Some(connector) = &package.package.connector else {
            continue;
        };
        let manifest = &package.package.manifest;
        let plugin = manifest.plugin_id().to_string();
        let Some(entry) = expected.get(plugin.as_str()) else {
            panic!("{plugin} carries a connector table and is not a listed bundled connector");
        };
        seen.insert(plugin.clone());
        // One package, one protocol, and the protocol its own application speaks. Two packages
        // that swapped their applications would each still be internally consistent.
        assert_eq!(
            connector.protocol.name, entry.protocol,
            "{plugin} is pinned to the wrong protocol"
        );
        // A profile is not selected by a name on disk alone. A package that reads somebody's
        // protocol carries a rule that identifies the application by something that cannot be
        // coincidence, and an executable name on its own is always inferred.
        let mut exact = 0;
        for rule in &manifest.match_rules {
            assert_eq!(
                rule.executable.file_stem, entry.file_stem,
                "{plugin}: the rule {} recognises another application's executable",
                rule.id
            );
            // A bundled connector recognises its application wherever it was installed, so a
            // directory requirement would make it miss ordinary installations.
            assert!(
                rule.executable.path_suffix.is_empty(),
                "{plugin}: the rule {} only matches under a directory",
                rule.id
            );
            match &rule.distribution.0 {
                None => assert_eq!(
                    rule.confidence,
                    MatchConfidence::Inferred,
                    "{plugin}: the rule {} claims an exact match from an executable name",
                    rule.id
                ),
                Some(distribution) => {
                    assert_eq!(
                        (distribution.registry(), distribution.identifier()),
                        (entry.registry, entry.identifier),
                        "{plugin}: the rule {} recognises another application's distribution",
                        rule.id
                    );
                    exact += usize::from(rule.confidence == MatchConfidence::Exact);
                }
            }
        }
        assert!(
            exact > 0,
            "{plugin} recognises its application by name alone"
        );
    }
    assert_eq!(
        seen.len(),
        BUNDLED_CONNECTORS.len(),
        "a listed bundled connector is not in the repository"
    );
}

/// One pinned vendor frame and what the package's table has to make of it.
#[derive(serde::Deserialize)]
struct CorpusFrame {
    situation: String,
    #[allow(dead_code)]
    note: String,
    frame: serde_json::Value,
    expect: Expectation,
}

#[derive(serde::Deserialize)]
struct Expectation {
    wire_name: String,
    /// The method the table names, or nothing when the table does not route it.
    method: Option<String>,
    direction: Option<String>,
    class: String,
    /// The identifier the table's path finds, with its JSON type, or nothing when there is none.
    request_id: Option<serde_json::Value>,
    /// Members the situation is about, by dotted path. A frame that loses one stops being an
    /// example of its situation, so they are asserted rather than left to a reader.
    #[serde(default)]
    members: BTreeMap<String, serde_json::Value>,
}

#[derive(serde::Deserialize)]
struct FrameCorpus {
    corpus_version: u32,
    protocol: String,
    tested_version: String,
    #[allow(dead_code)]
    source: String,
    frames: Vec<CorpusFrame>,
}

/// Walks a connector's bounded field path over a decoded frame.
fn at<'a>(value: &'a serde_json::Value, path: &FieldPath) -> Option<&'a serde_json::Value> {
    let mut current = value;
    for segment in &path.segments {
        current = match segment {
            FieldSegment::Member { name } => current.get(name)?,
            FieldSegment::Index { index } => current.get(*index as usize)?,
        };
    }
    Some(current)
}

/// Walks a dotted member path over a decoded frame.
fn member<'a>(value: &'a serde_json::Value, path: &str) -> Option<&'a serde_json::Value> {
    let mut current = value;
    for name in path.split('.') {
        current = current.get(name)?;
    }
    Some(current)
}

#[test]
fn every_pinned_frame_is_read_the_way_the_table_says() {
    let loaded = packages::load(&root()).expect("the repository loads");
    let mut checked = 0;
    for package in &loaded.repository.packages {
        let Some(connector) = &package.package.connector else {
            continue;
        };
        let path = package.directory.join("fixtures/frames.json");
        let corpus: FrameCorpus = kalareach_catalogue::read_json(&path)
            .unwrap_or_else(|error| panic!("{}: {error}", package.relative));
        assert_eq!(corpus.corpus_version, 1, "{}", package.relative);
        assert_eq!(
            corpus.protocol, connector.protocol.name,
            "{}: the corpus names another protocol",
            package.relative
        );
        assert_eq!(
            corpus.tested_version,
            connector.protocol.tested_version.to_string(),
            "{}: the corpus was taken from another version",
            package.relative
        );
        let entry = BUNDLED_CONNECTORS
            .iter()
            .find(|entry| entry.plugin == package.package.manifest.plugin_id().as_str())
            .unwrap_or_else(|| panic!("{} is not a listed bundled connector", package.relative));
        assert!(
            matches!(
                connector.response_correlation,
                ResponseCorrelation::MatchingId { .. }
            ),
            "{}: these connectors correlate a response by repeating the identifier",
            package.relative
        );
        let mut carried: Vec<(String, String)> = Vec::new();
        for case in &corpus.frames {
            carried.push((case.situation.clone(), case.expect.wire_name.clone()));
            let where_ = format!("{} [{}]", package.relative, case.situation);
            checked += 1;

            // The method the table finds, at the path the table declares.
            let wire = at(&case.frame, &connector.method_path)
                .and_then(serde_json::Value::as_str)
                .unwrap_or_else(|| panic!("{where_}: no method at the table's method path"));
            assert_eq!(wire, case.expect.wire_name, "{where_}");

            // The identifier the table finds, with the JSON type it was written with. A string
            // identifier and the number that spells the same digits are different requests.
            let found = at(&case.frame, &connector.request_id_path);
            assert_eq!(
                found,
                case.expect.request_id.as_ref(),
                "{where_}: the identifier at the table's path is not the one the corpus declares"
            );

            // The members the situation is about. Dropping `expectedTurnId` from a steer frame
            // leaves a frame that still routes and still carries an identifier, and stops being an
            // example of the thing its situation names.
            for (path, expected) in &case.expect.members {
                assert_eq!(
                    member(&case.frame, path),
                    Some(expected),
                    "{where_}: the frame's {path} is not what the corpus declares"
                );
            }

            // A response is matched by repeating the identifier, so the correlation path has to
            // find the same value the request path does.
            let ResponseCorrelation::MatchingId { id_path } = &connector.response_correlation
            else {
                unreachable!("the correlation mode was checked above")
            };
            assert_eq!(
                at(&case.frame, id_path),
                case.expect.request_id.as_ref(),
                "{where_}: the correlation path finds a different identifier from the request path"
            );

            // Members this test requires, whatever the corpus says about itself.
            for (wire_name, path) in entry.required_members {
                if *wire_name == case.expect.wire_name {
                    assert!(
                        member(&case.frame, path).is_some(),
                        "{where_}: the frame carries no {path}"
                    );
                }
            }

            match connector.route_for_wire_name(wire) {
                None => {
                    assert!(
                        case.expect.method.is_none(),
                        "{where_}: the corpus expects this to be routed and the table does not route it"
                    );
                    // An unrouted request is a mutation, and nothing in a table changes that.
                    assert_eq!(
                        case.expect.class, "mutation",
                        "{where_}: an unrouted method is a mutation"
                    );
                    assert_eq!(MethodClass::UNCLASSIFIED, MethodClass::Mutation);
                }
                Some(route) => {
                    assert_eq!(
                        Some(route.method.to_string()),
                        case.expect.method,
                        "{where_}: the table routes this to another method"
                    );
                    let direction = serde_json::to_value(route.direction)
                        .ok()
                        .and_then(|value| value.as_str().map(str::to_owned));
                    assert_eq!(direction, case.expect.direction, "{where_}");
                    let class = serde_json::to_value(connector.classify(&route.method))
                        .ok()
                        .and_then(|value| value.as_str().map(str::to_owned));
                    assert_eq!(class, Some(case.expect.class.clone()), "{where_}");
                }
            }
        }
        carried.sort();
        let declared: Vec<(&str, &str)> = carried
            .iter()
            .map(|(situation, wire)| (situation.as_str(), wire.as_str()))
            .collect();
        assert_eq!(
            declared, entry.frames,
            "{}: the corpus carries other frames than the ones it has to",
            package.relative
        );
    }
    assert!(checked > 0, "no package carries a frame corpus");
}

#[test]
fn a_tampered_package_is_rejected() {
    let source = root().join("plugins/kalareach/example-declarative");
    let temporary = tempfile::tempdir().expect("a temporary directory");
    let repository = temporary.path();
    std::fs::create_dir_all(repository.join("publishers")).expect("publishers directory");
    std::fs::copy(
        root().join("publishers/kalareach.json"),
        repository.join("publishers/kalareach.json"),
    )
    .expect("the publisher record copies");
    let destination = repository.join("plugins/kalareach/example-declarative");
    copy_tree(&source, &destination);

    // One byte of the presentation document changes, and nothing else.
    let presentation = destination.join("presentation.json");
    let mut text = std::fs::read_to_string(&presentation).expect("the document reads");
    text.push('\n');
    std::fs::write(&presentation, text).expect("the document writes");

    let loaded = packages::load(repository).expect("the repository loads");
    assert_eq!(loaded.rejected.len(), 1);
    let report = &loaded.rejected[0].report;
    assert!(report.has(kr_plugin_sdk::validate::FindingCode::DigestMismatch));
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
