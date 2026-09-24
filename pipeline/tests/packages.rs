//! Every package in the repository validates, and a broken one does not.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use kalareach_catalogue::{fixtures, packages, repository_root};
use kr_plugin_sdk::connector::{FieldPath, FieldSegment, MethodClass, ResponseCorrelation};
use kr_plugin_sdk::effect::{ActionImplementation, AttachmentInsertion, EffectClass};
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

/// How a package proves which application it recognised.
///
/// A registry or bundle identity cannot be a coincidence, so a rule may claim it exactly. An
/// application installed by its vendor has no such identity: the strongest thing a rule can name is
/// the directory the vendor documents, which is a guess and stays one.
enum ApplicationIdentity {
    /// The application is published in a registry, under these identities.
    Distribution {
        /// Registry and identifier pairs, at least one of which a rule must claim exactly.
        registries: &'static [(&'static str, &'static str)],
    },
    /// The application is installed by its vendor, into this directory.
    VendorInstaller {
        /// Whole path segments a rule must require, at least once.
        path_suffix: &'static [&'static str],
    },
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
        identity: ApplicationIdentity::Distribution {
            registries: &[("npm", "@anthropic-ai/claude-code")],
        },
        file_stems: &["claude"],
        provenance: &[],
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
        identity: ApplicationIdentity::Distribution {
            registries: &[("npm", "@openai/codex")],
        },
        file_stems: &["codex"],
        provenance: &[],
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
    BundledConnector {
        plugin: "kalareach/gemini-cli",
        protocol: "gemini-cli-acp",
        identity: ApplicationIdentity::Distribution {
            registries: &[("npm", "@google/gemini-cli")],
        },
        file_stems: &["gemini"],
        provenance: &[
            (
                "a file read in the agent's own environment",
                Provenance::Written,
            ),
            ("a method the table does not route", Provenance::Captured),
            (
                "a process started in the agent's own environment",
                Provenance::Written,
            ),
            ("a tool call the agent is waiting on", Provenance::Written),
            ("creating an execution", Provenance::AbridgedCapture),
            ("ending the turn in flight", Provenance::Written),
            ("the agent's own update stream", Provenance::AbridgedCapture),
            (
                "the capability negotiation that opens a connection",
                Provenance::Captured,
            ),
        ],
        frames: &[
            (
                "a file read in the agent's own environment",
                "fs/read_text_file",
            ),
            (
                "a method the table does not route",
                "kalareach/not-a-method",
            ),
            (
                "a process started in the agent's own environment",
                "terminal/create",
            ),
            (
                "a tool call the agent is waiting on",
                "session/request_permission",
            ),
            ("creating an execution", "session/new"),
            ("ending the turn in flight", "session/cancel"),
            ("the agent's own update stream", "session/update"),
            (
                "the capability negotiation that opens a connection",
                "initialize",
            ),
        ],
        required_members: &[
            ("fs/read_text_file", "params.path"),
            ("session/new", "params.cwd"),
            ("session/request_permission", "params.toolCall.toolCallId"),
            ("session/update", "params.update.sessionUpdate"),
            ("terminal/create", "params.command"),
        ],
    },
    BundledConnector {
        plugin: "kalareach/kimi-code-cli",
        protocol: "kimi-code-cli-acp",
        identity: ApplicationIdentity::VendorInstaller {
            path_suffix: &[".kimi-code", "bin"],
        },
        file_stems: &["kimi"],
        provenance: &[
            (
                "a file read in the agent's own environment",
                Provenance::Written,
            ),
            ("a method the table does not route", Provenance::Captured),
            (
                "a process started in the agent's own environment",
                Provenance::Written,
            ),
            ("a tool call the agent is waiting on", Provenance::Written),
            ("creating an execution", Provenance::AbridgedCapture),
            ("ending the turn in flight", Provenance::Written),
            ("the agent's own update stream", Provenance::Written),
            (
                "the capability negotiation that opens a connection",
                Provenance::Captured,
            ),
        ],
        frames: &[
            (
                "a file read in the agent's own environment",
                "fs/read_text_file",
            ),
            (
                "a method the table does not route",
                "kalareach/not-a-method",
            ),
            (
                "a process started in the agent's own environment",
                "terminal/create",
            ),
            (
                "a tool call the agent is waiting on",
                "session/request_permission",
            ),
            ("creating an execution", "session/new"),
            ("ending the turn in flight", "session/cancel"),
            ("the agent's own update stream", "session/update"),
            (
                "the capability negotiation that opens a connection",
                "initialize",
            ),
        ],
        required_members: &[
            ("fs/read_text_file", "params.path"),
            ("session/new", "params.cwd"),
            ("session/request_permission", "params.toolCall.toolCallId"),
            ("session/update", "params.update.sessionUpdate"),
            ("terminal/create", "params.command"),
        ],
    },
    BundledConnector {
        plugin: "kalareach/opencode",
        protocol: "opencode-server-v2-events",
        identity: ApplicationIdentity::Distribution {
            registries: &[("npm", "opencode-ai")],
        },
        file_stems: &["opencode"],
        provenance: &[
            (
                "a permission the session is waiting on",
                Provenance::Written,
            ),
            ("a running turn's text", Provenance::Written),
            (
                "a terminal command this connector refuses",
                Provenance::Written,
            ),
            ("an envelope from the other API family", Provenance::Written),
            (
                "an event only the other API family publishes",
                Provenance::Written,
            ),
            (
                "composer insertion this connector refuses",
                Provenance::Written,
            ),
            ("the answer that resolved it", Provenance::Written),
            ("the shared per-user scope disposed", Provenance::Written),
            (
                "the terminal moving to another session",
                Provenance::Written,
            ),
        ],
        frames: &[
            (
                "a permission the session is waiting on",
                "permission.v2.asked",
            ),
            ("a running turn's text", "session.next.text.delta"),
            (
                "a terminal command this connector refuses",
                "tui.command.execute",
            ),
            ("an envelope from the other API family", "permission.asked"),
            (
                "an event only the other API family publishes",
                "server.instance.disposed",
            ),
            (
                "composer insertion this connector refuses",
                "tui.prompt.append",
            ),
            ("the answer that resolved it", "permission.v2.replied"),
            ("the shared per-user scope disposed", "global.disposed"),
            (
                "the terminal moving to another session",
                "tui.session.select",
            ),
        ],
        required_members: &[
            ("permission.v2.asked", "data.id"),
            ("permission.v2.replied", "data.requestID"),
            ("session.next.text.delta", "data.sessionID"),
            ("tui.command.execute", "data.command"),
            ("tui.prompt.append", "data.text"),
            ("tui.session.select", "data.sessionID"),
        ],
    },
    BundledConnector {
        plugin: "kalareach/opencode-attach",
        protocol: "opencode-server-events",
        identity: ApplicationIdentity::Distribution {
            registries: &[("npm", "opencode-ai")],
        },
        file_stems: &["opencode"],
        provenance: &[],
        frames: &[
            ("a permission the session is waiting on", "permission.asked"),
            ("a running turn's text", "message.part.delta"),
            (
                "a terminal command this connector refuses",
                "tui.command.execute",
            ),
            (
                "an envelope from the shared-server API family",
                "permission.v2.asked",
            ),
            (
                "an event only this API family publishes",
                "server.instance.disposed",
            ),
            (
                "composer insertion this connector refuses",
                "tui.prompt.append",
            ),
            ("the answer that resolved it", "permission.replied"),
            ("the server's global scope disposed", "global.disposed"),
            (
                "the terminal moving to another session",
                "tui.session.select",
            ),
        ],
        required_members: &[
            ("message.part.delta", "properties.sessionID"),
            ("permission.asked", "properties.id"),
            ("permission.replied", "properties.requestID"),
            ("server.instance.disposed", "properties.directory"),
            ("tui.command.execute", "properties.command"),
            ("tui.prompt.append", "properties.text"),
            ("tui.session.select", "properties.sessionID"),
        ],
    },
    BundledConnector {
        plugin: "kalareach/qoder-cli",
        protocol: "qoder-cli-acp",
        identity: ApplicationIdentity::VendorInstaller {
            path_suffix: &[".qoder", "entry"],
        },
        file_stems: &["qoder", "qodercli", "qodercli-1.1.59"],
        provenance: &[
            (
                "a file read in the agent's own environment",
                Provenance::Written,
            ),
            ("a method the table does not route", Provenance::Captured),
            (
                "a process started in the agent's own environment",
                Provenance::Written,
            ),
            ("a tool call the agent is waiting on", Provenance::Written),
            ("creating an execution", Provenance::AbridgedCapture),
            ("ending the turn in flight", Provenance::Written),
            ("the agent's own update stream", Provenance::Written),
            (
                "the capability negotiation that opens a connection",
                Provenance::Captured,
            ),
        ],
        frames: &[
            (
                "a file read in the agent's own environment",
                "fs/read_text_file",
            ),
            (
                "a method the table does not route",
                "kalareach/not-a-method",
            ),
            (
                "a process started in the agent's own environment",
                "terminal/create",
            ),
            (
                "a tool call the agent is waiting on",
                "session/request_permission",
            ),
            ("creating an execution", "session/new"),
            ("ending the turn in flight", "session/cancel"),
            ("the agent's own update stream", "session/update"),
            (
                "the capability negotiation that opens a connection",
                "initialize",
            ),
        ],
        required_members: &[
            ("fs/read_text_file", "params.path"),
            ("session/new", "params.cwd"),
            ("session/request_permission", "params.toolCall.toolCallId"),
            ("session/update", "params.update.sessionUpdate"),
            ("terminal/create", "params.command"),
        ],
    },
];

struct BundledConnector {
    plugin: &'static str,
    protocol: &'static str,
    /// What the package's match rules have to prove about the application they recognised.
    identity: ApplicationIdentity,
    /// The executable names the package's rules may recognise, and no others. A vendor that puts
    /// the release number in the installed file name is recognised by that whole name, so a
    /// version appears here for the release its package is qualified against.
    file_stems: &'static [&'static str],
    /// Every frame the corpus has to carry, as a situation and a wire name, in sorted order.
    frames: &'static [(&'static str, &'static str)],
    /// Where each frame came from, as a situation and a provenance, in sorted order. Provenance
    /// is a claim about evidence, so every frame's is pinned: a corpus that promoted a written
    /// frame to a captured one, or quietly demoted a captured one, would otherwise pass. An empty
    /// list says every frame in that corpus is written, which is the weakest claim a corpus can
    /// make and the one a corpus that says nothing is held to.
    provenance: &'static [(&'static str, Provenance)],
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
        // for two facts, and both have to be asserted rather than mentioned. Substring checks
        // alone could accept a crafted negation (e.g. "runs under Claude Code's own permissions
        // inside the sandbox; it never runs outside Wasmtime's sandbox"). To ensure honest and
        // complete disclosure, the Claude Code grant is compared directly against the reviewed
        // disclosure with assert_eq!.
        let statement = bridge.grant_statement.as_str();
        if package.package.manifest.plugin_id().as_str() == "kalareach/claude-code" {
            assert_eq!(
                statement,
                "Installs three registration files under your own Claude Code directory, where \
                 they apply to every project and every later session, and adds one settings key \
                 that enables them. Claude Code then starts the KalaReach forwarder itself, so the \
                 forwarder runs under Claude Code's own permissions and outside the KalaReach \
                 plugin sandbox, outside Wasmtime. It sees the session's SessionStart, SessionEnd, \
                 PostToolUse, PostToolUseFailure and Notification events, and every Channels tool \
                 approval relayed to it. Removal takes the key back out and deletes exactly those \
                 three files, each only while it still holds the bytes that were installed."
            );
        } else {
            assert!(
                statement.contains("runs under")
                    && statement.contains("own permissions")
                    && statement.contains("outside the KalaReach plugin sandbox, outside Wasmtime")
                    && statement.contains("Removal"),
                "{}: grant statement does not disclose required sandbox and removal terms: {statement}",
                package.relative
            );
        }
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
        // protocol carries a rule that identifies the application by something stronger than its
        // executable name, and a rule that has only the name is always inferred.
        let mut exact = 0;
        let mut directories = 0;
        let mut anywhere = 0;
        for rule in &manifest.match_rules {
            assert!(
                entry
                    .file_stems
                    .contains(&rule.executable.file_stem.as_str()),
                "{plugin}: the rule {} recognises another application's executable",
                rule.id
            );
            if rule.executable.path_suffix.is_empty() {
                anywhere += 1;
            } else {
                directories += 1;
            }
            match &rule.distribution.0 {
                None => assert_eq!(
                    rule.confidence,
                    MatchConfidence::Inferred,
                    "{plugin}: the rule {} claims an exact match without a distribution",
                    rule.id
                ),
                Some(distribution) => {
                    let named = (distribution.registry(), distribution.identifier());
                    let ApplicationIdentity::Distribution { registries } = &entry.identity else {
                        panic!(
                            "{plugin}: the rule {} names a registry this application is not published in",
                            rule.id
                        )
                    };
                    assert!(
                        registries.contains(&named),
                        "{plugin}: the rule {} recognises another application's distribution",
                        rule.id
                    );
                    exact += usize::from(rule.confidence == MatchConfidence::Exact);
                }
            }
        }
        // Whatever else it carries, a bundled connector keeps one rule that recognises its
        // application wherever it was installed. A package whose every rule required a directory
        // would miss an ordinary installation and leave the agent unrecognised.
        assert!(
            anywhere > 0,
            "{plugin}: every rule requires a directory, so an ordinary installation matches none"
        );
        match &entry.identity {
            ApplicationIdentity::Distribution { .. } => {
                assert!(
                    exact > 0,
                    "{plugin} recognises its application by name alone"
                );
                // A published application is recognised wherever it was installed. A directory
                // requirement on any rule would make that rule miss ordinary installations, and
                // the exact one missing them is the whole of the package's proof gone.
                assert_eq!(
                    directories, 0,
                    "{plugin} has a rule that only matches under a directory"
                );
            }
            ApplicationIdentity::VendorInstaller { path_suffix } => {
                // A vendor installer has no registry identity to claim, so nothing here may be
                // exact and the directory the vendor documents is the strongest rule available.
                assert_eq!(
                    exact, 0,
                    "{plugin} claims an exact match for an application no registry publishes"
                );
                assert!(
                    directories > 0,
                    "{plugin} names no installation directory, so it recognises its application by name alone"
                );
                assert!(
                    manifest
                        .match_rules
                        .iter()
                        .any(|rule| rule.executable.path_suffix == *path_suffix),
                    "{plugin} names another installation directory than the one it was qualified against"
                );
            }
        }
    }
    assert_eq!(
        seen.len(),
        BUNDLED_CONNECTORS.len(),
        "a listed bundled connector is not in the repository"
    );
}

/// Where a pinned frame came from.
///
/// A frame that crossed a real connection is worth more than one written from a schema, and a
/// reader can only tell them apart if the corpus says which is which. The default is the weakest
/// answer, so a corpus that says nothing claims nothing.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, serde::Deserialize)]
#[serde(rename_all = "snake_case")]
enum Provenance {
    /// Written from a published schema or document, and never sent or received.
    #[default]
    Written,
    /// Exactly what crossed the connection.
    Captured,
    /// What crossed the connection, with a repeated part shortened and the corpus saying so.
    AbridgedCapture,
}

/// One pinned vendor frame and what the package's table has to make of it.
#[derive(serde::Deserialize)]
struct CorpusFrame {
    situation: String,
    #[allow(dead_code)]
    note: String,
    #[serde(default)]
    provenance: Provenance,
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

            // Provenance is a claim about evidence, so it is pinned rather than trusted, and all
            // three states are pinned: promoting a written frame and demoting a captured one are
            // the same kind of mistake.
            let declared = if entry.provenance.is_empty() {
                Provenance::Written
            } else {
                entry
                    .provenance
                    .iter()
                    .find(|(situation, _)| *situation == case.situation)
                    .map(|(_, provenance)| *provenance)
                    .unwrap_or_else(|| {
                        panic!("{where_}: the repository records no provenance for it")
                    })
            };
            assert_eq!(
                case.provenance, declared,
                "{where_}: the corpus and this test disagree about where the frame came from"
            );

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

/// One frame from an API family a table is not qualified for, and what the table makes of it.
struct SecondFamilyFrame {
    plugin: &'static str,
    situation: &'static str,
    /// The member the pinned family's payload would be under, and which this frame does not carry.
    absent_member: &'static str,
    /// The member the other family puts its payload under, which is where this frame's own
    /// expectation has to read, because reading anywhere else would be reading this family's shape.
    payload_member: &'static str,
    /// The class the table gives this frame.
    class: &'static str,
    /// True when the name belongs to the other family alone, which the table may refuse outright.
    refused_by_name: bool,
}

/// The frames that belong to another profile of the same application.
///
/// OpenCode's server answers two API families from one build. They share almost every event name
/// and differ in where the payload sits, so a name settles nothing: the profile is chosen from the
/// installed version and the served schema before a connection opens. What a table can say is this.
/// A name only the other family publishes is declared unsupported, so seeing it is an answer rather
/// than a default. A shared name routes and carries nothing under the member this table reads, so
/// nothing comes out of it. Both are pinned here, because the second is the weaker of the two and a
/// reader should not have to guess which case they are looking at.
const SECOND_FAMILY_FRAMES: &[SecondFamilyFrame] = &[
    SecondFamilyFrame {
        plugin: "kalareach/opencode",
        situation: "an envelope from the other API family",
        absent_member: "data",
        payload_member: "properties",
        class: "observation",
        refused_by_name: false,
    },
    SecondFamilyFrame {
        plugin: "kalareach/opencode",
        situation: "an event only the other API family publishes",
        absent_member: "data",
        payload_member: "properties",
        class: "unsupported",
        refused_by_name: true,
    },
    SecondFamilyFrame {
        plugin: "kalareach/opencode-attach",
        situation: "an envelope from the shared-server API family",
        absent_member: "properties",
        payload_member: "data",
        class: "observation",
        refused_by_name: false,
    },
];

/// A name only one API family publishes, pinned in the corpus of the package for that family.
struct OwnFamilyFrame {
    plugin: &'static str,
    situation: &'static str,
    /// The member this family puts its payload under, which the frame has to carry.
    payload_member: &'static str,
    /// The package for the other family, whose table has to refuse this name.
    other_plugin: &'static str,
}

/// The names that tell two API families apart from the side of the family that publishes them.
///
/// OpenCode's earlier family publishes one event name the shared-server family does not, and the
/// shared-server family publishes none the earlier one lacks. So the earlier family's package has no
/// name it could refuse, and what tells the families apart there is its own exclusive name: its table
/// reads it as the event it is, and the shared-server package refuses the same name outright.
const OWN_FAMILY_FRAMES: &[OwnFamilyFrame] = &[OwnFamilyFrame {
    plugin: "kalareach/opencode-attach",
    situation: "an event only this API family publishes",
    payload_member: "properties",
    other_plugin: "kalareach/opencode",
}];

#[test]
fn a_frame_from_the_other_api_family_is_unsupported_or_carries_no_payload() {
    let loaded = packages::load(&root()).expect("the repository loads");
    let mut checked = 0;
    for expected in SECOND_FAMILY_FRAMES {
        let package = loaded
            .repository
            .packages
            .iter()
            .find(|package| package.package.manifest.plugin_id().as_str() == expected.plugin)
            .unwrap_or_else(|| panic!("{} is not in the repository", expected.plugin));
        let connector = package
            .package
            .connector
            .as_ref()
            .unwrap_or_else(|| panic!("{} carries no connector table", expected.plugin));
        let corpus: FrameCorpus =
            kalareach_catalogue::read_json(&package.directory.join("fixtures/frames.json"))
                .unwrap_or_else(|error| panic!("{}: {error}", package.relative));
        let case = corpus
            .frames
            .iter()
            .find(|case| case.situation == expected.situation)
            .unwrap_or_else(|| panic!("{}: no frame for {}", package.relative, expected.situation));
        checked += 1;
        let where_ = format!("{} [{}]", package.relative, expected.situation);

        // The payload member this table reads is simply not there. Everything this table would go
        // on to extract comes from under it, so nothing comes out of the frame.
        assert!(
            member(&case.frame, expected.absent_member).is_none(),
            "{where_}: the frame carries {}, so it is not from the other family",
            expected.absent_member
        );
        let wire = at(&case.frame, &connector.method_path)
            .and_then(serde_json::Value::as_str)
            .unwrap_or_else(|| panic!("{where_}: no method at the table's method path"));
        let route = connector.route_for_wire_name(wire).unwrap_or_else(|| {
            panic!("{where_}: the table has to name this event to answer for it")
        });
        let class = serde_json::to_value(connector.classify(&route.method))
            .ok()
            .and_then(|value| value.as_str().map(str::to_owned));
        assert_eq!(
            class.as_deref(),
            Some(expected.class),
            "{where_}: the table gives this frame another class than the repository expects"
        );
        if expected.refused_by_name {
            // A name only the other family publishes is the one case a table can refuse outright,
            // and saying so is worth more than letting it fall to the unclassified default.
            assert_eq!(
                expected.class, "unsupported",
                "{where_}: a name exclusive to the other family is refused by name"
            );
            assert!(
                !connector
                    .methods
                    .iter()
                    .any(|entry| entry.method.as_str() == wire
                        && entry.class != MethodClass::Unsupported),
                "{where_}: the table classifies this name twice"
            );
        } else {
            // A shared name is the case a table cannot refuse: it routes, and everything under it
            // sits where this table never looks. The corpus's own expectation has to say so,
            // rather than quietly describing this family's shape on the other family's frame.
            assert!(
                !case.expect.members.is_empty(),
                "{where_}: a frame that proves nothing about its payload proves nothing"
            );
            for path in case.expect.members.keys() {
                assert!(
                    path.starts_with(&format!("{}.", expected.payload_member)),
                    "{where_}: the expectation reads {path}, which is not where this frame's family puts its payload"
                );
            }
            // A shared name settles nothing, so the package also pins the name that does: the
            // other family's, which its table refuses, or where the other family has none, its own.
            assert!(
                SECOND_FAMILY_FRAMES
                    .iter()
                    .any(|other| other.plugin == expected.plugin && other.refused_by_name)
                    || OWN_FAMILY_FRAMES
                        .iter()
                        .any(|own| own.plugin == expected.plugin),
                "{where_}: a package that pins a shared name also pins the name that tells the families apart"
            );
        }
    }
    assert_eq!(checked, SECOND_FAMILY_FRAMES.len());
}

#[test]
fn a_name_only_one_api_family_publishes_is_read_by_its_own_table_and_refused_by_the_other() {
    let loaded = packages::load(&root()).expect("the repository loads");
    let connector_of = |plugin: &str| {
        let package = loaded
            .repository
            .packages
            .iter()
            .find(|package| package.package.manifest.plugin_id().as_str() == plugin)
            .unwrap_or_else(|| panic!("{plugin} is not in the repository"));
        let connector = package
            .package
            .connector
            .as_ref()
            .unwrap_or_else(|| panic!("{plugin} carries no connector table"));
        (package, connector)
    };
    let class_of = |connector: &kr_plugin_sdk::connector::ConnectorManifest, wire: &str| {
        connector.route_for_wire_name(wire).map(|route| {
            serde_json::to_value(connector.classify(&route.method))
                .ok()
                .and_then(|value| value.as_str().map(str::to_owned))
        })
    };
    let mut checked = 0;
    for expected in OWN_FAMILY_FRAMES {
        let (package, connector) = connector_of(expected.plugin);
        let corpus: FrameCorpus =
            kalareach_catalogue::read_json(&package.directory.join("fixtures/frames.json"))
                .unwrap_or_else(|error| panic!("{}: {error}", package.relative));
        let case = corpus
            .frames
            .iter()
            .find(|case| case.situation == expected.situation)
            .unwrap_or_else(|| panic!("{}: no frame for {}", package.relative, expected.situation));
        checked += 1;
        let where_ = format!("{} [{}]", package.relative, expected.situation);

        // The frame is this family's own shape, so its payload is where this table reads it.
        assert!(
            member(&case.frame, expected.payload_member).is_some(),
            "{where_}: the frame carries no {}, so it is not this family's",
            expected.payload_member
        );
        let wire = at(&case.frame, &connector.method_path)
            .and_then(serde_json::Value::as_str)
            .unwrap_or_else(|| panic!("{where_}: no method at the table's method path"));

        // Its own table reads it as the event it is. Refusing a name its own family publishes
        // would refuse the evidence that the connection is the family the table was qualified for.
        let own = class_of(connector, wire)
            .unwrap_or_else(|| panic!("{where_}: the table does not route its own family's name"));
        assert_ne!(
            own.as_deref(),
            Some("unsupported"),
            "{where_}: the table refuses a name its own family publishes"
        );
        assert!(
            case.expect
                .members
                .keys()
                .all(|path| path.starts_with(&format!("{}.", expected.payload_member))),
            "{where_}: the expectation reads outside this family's payload"
        );

        // The other family's package refuses the same name outright, and pins that it does.
        let (other_package, other_connector) = connector_of(expected.other_plugin);
        assert_eq!(
            class_of(other_connector, wire).flatten().as_deref(),
            Some("unsupported"),
            "{where_}: {} does not refuse {wire}",
            other_package.relative
        );
        let other_corpus: FrameCorpus =
            kalareach_catalogue::read_json(&other_package.directory.join("fixtures/frames.json"))
                .unwrap_or_else(|error| panic!("{}: {error}", other_package.relative));
        assert!(
            SECOND_FAMILY_FRAMES.iter().any(|refused| {
                refused.plugin == expected.other_plugin
                    && refused.refused_by_name
                    && other_corpus.frames.iter().any(|other_case| {
                        other_case.situation == refused.situation
                            && other_case.expect.wire_name == wire
                    })
            }),
            "{where_}: {} pins no frame refusing {wire}",
            other_package.relative
        );
    }
    assert_eq!(checked, OWN_FAMILY_FRAMES.len());
}

/// Every agent section 12 bundles, and the plugin that carries its adapter.
///
/// This reads manifests. It says that no adapter was dropped, that no two adapters claim one
/// protocol, and that nothing a control invokes reaches past what that package's own qualified
/// table routes. That a host actually launches the terminal route, and that an agent protocol is
/// entered only when somebody selects it, are the host's and are not settled here.
const BUNDLED_AGENTS: &[&str] = &[
    "kalareach/claude-code",
    "kalareach/codex",
    "kalareach/gemini-cli",
    "kalareach/kimi-code-cli",
    "kalareach/opencode",
    "kalareach/qoder-cli",
];

#[test]
fn every_bundled_adapter_is_present_and_declares_no_surface_of_its_own() {
    let loaded = packages::load(&root()).expect("the repository loads");
    let mut protocols: BTreeMap<String, &str> = BTreeMap::new();
    for agent in BUNDLED_AGENTS {
        // A missing upstream API is not a reason to drop an adapter, so every bundled agent has
        // one in the repository whatever its protocol turned out to support.
        let package = loaded
            .repository
            .packages
            .iter()
            .find(|package| package.package.manifest.plugin_id().as_str() == *agent)
            .unwrap_or_else(|| panic!("{agent} has no package, so its adapter was dropped"));
        let manifest = &package.package.manifest;

        // Each adapter reads one vendor's own protocol. Two adapters sharing a pin would be one
        // control surface wearing two names, which is what a universal typed surface looks like
        // from here.
        if let Some(connector) = &package.package.connector {
            let name = connector.protocol.name.clone();
            if let Some(other) = protocols.insert(name.clone(), agent) {
                panic!("{agent} and {other} both claim the protocol {name}");
            }
        }

        // Automatic native-composer insertion needs the bridge that makes it safe. Without one, a
        // package may declare a typed or a manual path, and not that one.
        if let Some(attachments) = &manifest.attachments.0 {
            assert!(
                attachments.insertion != AttachmentInsertion::NativeComposer
                    || manifest.native_bridge.0.is_some(),
                "{agent} claims native-composer insertion without a bridge"
            );
        }

        for action in &manifest.actions {
            // Nothing a control invokes writes into the terminal. The terminal stays the person's,
            // operated through the input grant they already hold.
            assert_ne!(
                action.effect,
                EffectClass::TerminalInput,
                "{agent}: the action {} types into the native terminal",
                action.id
            );
            match &action.implementation {
                ActionImplementation::Presentation {} | ActionImplementation::UpstreamCancel {} => {
                }
                ActionImplementation::UpstreamMethod { method, .. } => {
                    // An upstream method is one this package's own qualified table routes. A
                    // method invented in a manifest would be a typed surface nobody qualified.
                    let connector = package
                        .package
                        .connector
                        .as_ref()
                        .unwrap_or_else(|| panic!("{agent} sends {method} with no table"));
                    assert!(
                        connector.routes.iter().any(|route| &route.method == method),
                        "{agent}: the action {} sends {method}, which its own table does not route",
                        action.id
                    );
                }
                other => panic!(
                    "{agent}: the action {} is implemented as {}, which is not a declarative form this repository reviewed",
                    action.id,
                    other.kind()
                ),
            }
        }
    }
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
