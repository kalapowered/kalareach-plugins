//! Every package in the repository validates, and a broken one does not.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use kalareach_catalogue::{fixtures, packages, repository_root};
use kr_plugin_sdk::capability::PluginCapability;
use kr_plugin_sdk::connector::{
    AnswerError, FieldPath, FieldSegment, MethodClass, ResponseCorrelation,
};
use kr_plugin_sdk::digest::PayloadDigest;
use kr_plugin_sdk::effect::{
    ActionImplementation, ActionInvocation, ActionRight, AttachmentInsertion, EffectClass,
    InvocationError, ParameterKind,
};
use kr_plugin_sdk::ids::ParameterName;
use kr_plugin_sdk::integration::{CommandIntegration, IntegrationVariable};
use kr_plugin_sdk::matching::MatchConfidence;
use kr_plugin_sdk::predicate::{BindingState, Predicate, PresentationFlag};
use kr_plugin_sdk::presentation::Control;
use kr_plugin_sdk::text::Summary;
use kr_plugin_sdk::validate::FindingCode;

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
        required_rules: &[],
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
        required_rules: &[],
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
        required_rules: &[],
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
        plugin: "kalareach/kimi-cli",
        protocol: "kimi-cli-acp",
        identity: ApplicationIdentity::Distribution {
            registries: &[("py_pi", "kimi-cli")],
        },
        file_stems: &["kimi", "kimi-cli"],
        required_rules: &[],
        provenance: &[
            (
                "a file read in the agent's own environment",
                Provenance::Written,
            ),
            (
                "a method only the other distribution implements",
                Provenance::Captured,
            ),
            ("a method the table does not route", Provenance::Captured),
            (
                "a method this build does not implement",
                Provenance::AbridgedCapture,
            ),
            (
                "a process started in the agent's own environment",
                Provenance::Written,
            ),
            ("a tool call the agent is waiting on", Provenance::Written),
            ("creating an execution", Provenance::Captured),
            ("ending the turn in flight", Provenance::Captured),
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
                "a method only the other distribution implements",
                "session/delete",
            ),
            (
                "a method the table does not route",
                "kalareach/not-a-method",
            ),
            ("a method this build does not implement", "session/fork"),
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
            ("session/cancel", "params.sessionId"),
            ("session/fork", "params.sessionId"),
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
        required_rules: &[],
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
        required_rules: &[],
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
        required_rules: &[],
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
        file_stems: &["qoder", "qodercli", "qodercli-1.1.63"],
        required_rules: &[("qodercli-1.1.63", &[".qoder", "bin", "qodercli"])],
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
    /// Rules the package has to carry, by file stem and path suffix, because its qualification
    /// depends on them. `file_stems` bounds what a rule may recognise; this says which rules must
    /// be there, so a package that lost one fails rather than passing on the rules that remain.
    required_rules: &'static [(&'static str, &'static [&'static str])],
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

/// The installation grant of every package that ships a native bridge, exactly as it was reviewed.
///
/// The validator checks that a recipe is complete and removable. It cannot check that the grant a
/// person reads before accepting it says what accepting it means. Section 11 asks for two facts,
/// that the bridge runs under the application's own permissions and that it runs outside Wasmtime,
/// and both have to be asserted rather than mentioned. No phrase test can tell the two apart: a
/// grant can carry every phrase a disclosure needs and deny each fact. So a reviewer reads each
/// statement, it is written here, and the package has to carry exactly that statement. A new bridge
/// package, or a changed statement, fails until its statement is reviewed and written here.
const REVIEWED_GRANTS: &[(&str, &str)] = &[
    (
        "kalareach/claude-code",
        "Installs three registration files under your own Claude Code directory, where they apply \
         to every project and every later session, and adds one settings key that enables them. \
         Each file names the KalaReach forwarder by the full path of the copy KalaReach installed, \
         which KalaReach writes into the file when it installs it. Claude Code then starts the \
         KalaReach forwarder itself, so the forwarder runs under Claude Code's own permissions and \
         outside the KalaReach plugin sandbox, outside Wasmtime. It sees the session's \
         SessionStart, SessionEnd, PostToolUse, PostToolUseFailure and Notification events, and \
         every Channels tool approval relayed to it. Removal takes the key back out and deletes \
         exactly those three files, each only while it still holds the bytes that were installed.",
    ),
    (
        "kalareach/gemini-cli",
        "Installs three files under your own Gemini CLI directory: the manifest of an extension \
         named kalareach, its hooks, and the record of where it is installed. Gemini CLI loads the \
         extension for every project and every later session, whether or not you trust the folder, \
         beside your own hooks; where your settings allow only listed extensions, it loads it only \
         when one of your patterns matches the source that record names. Gemini CLI then starts \
         the KalaReach forwarder itself, through bash, by the full path KalaReach writes in, so \
         the forwarder runs under Gemini CLI's own permissions and outside the KalaReach plugin \
         sandbox, outside Wasmtime. It sees each session's SessionStart, SessionEnd and \
         Notification events and no tool events, because Gemini CLI reads a failing hook's error \
         text as a refusal of a finished tool's result. Removal deletes exactly those three files, \
         each only while it still holds the bytes that were installed, and the record last, only \
         once nothing else is left beside it.",
    ),
];

/// Why a bridge package's installation grant is refused, or nothing when it is the reviewed one.
fn grant_refusal(plugin: &str, statement: &str) -> Option<String> {
    let Some((_, reviewed)) = REVIEWED_GRANTS
        .iter()
        .find(|(reviewed_plugin, _)| *reviewed_plugin == plugin)
    else {
        return Some(format!(
            "{plugin} ships a native bridge whose grant nobody has reviewed: {statement}"
        ));
    };
    (statement != *reviewed)
        .then(|| format!("{plugin}'s grant is not the reviewed statement: {statement}"))
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
        let plugin = package.package.manifest.plugin_id().to_string();
        if let Some(refusal) = grant_refusal(&plugin, bridge.grant_statement.as_str()) {
            panic!("{}: {refusal}", package.relative);
        }
    }
    assert!(recipes > 0, "no package ships a native bridge");
}

/// A bridge grant that carries every phrase the disclosure needs and denies each fact is refused.
#[test]
fn a_bridge_grant_that_denies_where_its_code_runs_is_refused() {
    let denied = "Installs one registration file. The forwarder never runs under its own \
                  permissions, and it never runs outside the KalaReach plugin sandbox, outside \
                  Wasmtime. Removal leaves the file where it is.";
    assert!(
        grant_refusal("kalareach/example-bridge", denied).is_some(),
        "a grant that denies where the bridge runs is accepted"
    );

    // A statement is reviewed for one package: the same words under another package's name, or one
    // word changed under its own, are not the reviewed grant.
    let (plugin, reviewed) = REVIEWED_GRANTS[0];
    assert_eq!(grant_refusal(plugin, reviewed), None);
    assert!(grant_refusal("kalareach/example-bridge", reviewed).is_some());
    let changed = reviewed.replacen("outside the KalaReach", "inside the KalaReach", 1);
    assert_ne!(changed, reviewed);
    assert!(grant_refusal(plugin, &changed).is_some());
}

/// The flags a reviewed command integration adds, in the order the host adds them.
enum ReviewedFlags {
    /// Each flag, written out.
    Written(&'static [&'static str]),
    /// Flags the core repository keeps as a fixture: how many there are, and the SHA-256 of the
    /// fixture, which is the list written as JSON with two-space indentation and a final newline.
    /// The package adds exactly the bytes the core repository's tests read.
    Pinned { count: usize, sha256: &'static str },
}

/// A command integration exactly as it was reviewed.
struct ReviewedIntegration {
    plugin: &'static str,
    command: &'static str,
    flags: ReviewedFlags,
    variables: &'static [(&'static str, &'static str)],
    /// What the package's grant says beside the list the host renders from the declaration.
    grant_statement: &'static str,
    /// The list the host renders from the declaration for the grant, exactly as the owner reads it.
    listed: &'static str,
}

/// The command integration of every package that declares one, exactly as it was reviewed.
///
/// The validator checks that a declaration's command is one of the package's own executables, that
/// each flag is one whole visible argument and that each variable is on the contract's closed list.
/// It cannot check that the flags are the ones the application's qualification found, or that the
/// statement a person reads before granting the integration says what granting it costs them. So
/// each declaration is written here, and the package has to carry exactly that: a new declaration,
/// or a changed command, flag, variable or statement, fails until it is reviewed and written here.
const REVIEWED_INTEGRATIONS: &[ReviewedIntegration] = &[
    ReviewedIntegration {
        plugin: "kalareach/claude-code",
        command: "claude",
        flags: ReviewedFlags::Written(&[
            "--dangerously-load-development-channels",
            "plugin:kalareach-channels@skills-dir",
        ]),
        variables: &[],
        grant_statement: "When you run claude in a KalaReach session with this integration on, \
             KalaReach adds the development channels flag naming kalareach-channels, the channel \
             plugin this package's bridge installs, so that Claude Code can register it as the \
             session's channel. The flag skips the channel allowlist for that one plugin; every \
             other condition Claude Code sets for a channel, your organisation's channel setting \
             among them, still applies. Claude Code shows a warning that lists the development \
             channel and asks you to confirm it in the terminal before the session starts.",
        listed: concat!(
            r#"Runs "claude" in KalaReach sessions with these arguments added, in this order: "#,
            r#""--dangerously-load-development-channels" "plugin:kalareach-channels@skills-dir"."#,
            r#" It sets no environment variables."#,
        ),
    },
    ReviewedIntegration {
        plugin: "kalareach/gemini-cli",
        command: "gemini",
        flags: ReviewedFlags::Written(&[]),
        variables: &[("GEMINI_CLI_NO_RELAUNCH", "true")],
        grant_statement: "When you run gemini in a KalaReach session with this integration on, \
             KalaReach sets GEMINI_CLI_NO_RELAUNCH to true for it, so the process KalaReach \
             launched runs the session itself instead of starting a second copy of Gemini CLI to \
             run it, and the hooks this package's bridge registers can select the session's \
             thread. You give up what that second copy was for. Gemini CLI no longer raises the \
             session's memory limit to half of this machine's memory, which it otherwise does \
             where that is more than Node's default, so a very large session runs out of memory \
             sooner. When Gemini CLI restarts after an update, or for a restart it asks for, the \
             session ends instead and you run gemini again.",
        listed: concat!(
            r#"Runs "gemini" in KalaReach sessions with no arguments added."#,
            r#" It sets these environment variables: GEMINI_CLI_NO_RELAUNCH="true"."#,
        ),
    },
    ReviewedIntegration {
        plugin: "kalareach/qoder-cli",
        command: "qodercli",
        // `--settings` and the inline settings after it, as the core repository keeps them in
        // `fixtures/bridges/qoder-cli/flags.json`.
        flags: ReviewedFlags::Pinned {
            count: 2,
            sha256: "4a23ebef3076b3d7f5aaa817d3f4f78561c48db869c6147b92a503e0c3fb6c23",
        },
        variables: &[],
        grant_statement: "When you run qodercli in a KalaReach session with this integration on, \
             KalaReach adds --settings with inline settings that register the KalaReach forwarder, \
             by the full path of the copy KalaReach installed, with the arguments qoder-cli hook, \
             for SessionStart, SessionEnd, PostToolUse, PostToolUseFailure and Notification, with \
             a timeout of one second for SessionEnd and five for the others. Qoder CLI runs these \
             hooks beside your own, and nothing is written to your Qoder CLI settings. Qoder CLI \
             starts the forwarder itself, so the forwarder runs under Qoder CLI's own permissions \
             and outside the KalaReach plugin sandbox, outside Wasmtime. It sees those five events \
             of that session.",
        // The inline settings are one argument, so they are written as one JSON string, their own
        // quotes escaped.
        listed: concat!(
            r#"Runs "qodercli" in KalaReach sessions with these arguments added,"#,
            r#" in this order: "--settings" "{\"hooks\":{\"SessionStart\":["#,
            r#"{\"hooks\":[{\"type\":\"command\",\"command\":\"{kr_hook}\",\"args\":["#,
            r#"\"qoder-cli\",\"hook\"],\"timeout\":5}]}],\"SessionEnd\":[{\"hooks\":["#,
            r#"{\"type\":\"command\",\"command\":\"{kr_hook}\",\"args\":[\"qoder-cli\","#,
            r#"\"hook\"],\"timeout\":1}]}],\"PostToolUse\":[{\"hooks\":[{\"type\":\"command\","#,
            r#"\"command\":\"{kr_hook}\",\"args\":[\"qoder-cli\",\"hook\"],\"timeout\":5}]}],"#,
            r#"\"PostToolUseFailure\":[{\"hooks\":[{\"type\":\"command\",\"command\":\"{kr_hook}\","#,
            r#"\"args\":[\"qoder-cli\",\"hook\"],\"timeout\":5}]}],\"Notification\":["#,
            r#"{\"hooks\":[{\"type\":\"command\",\"command\":\"{kr_hook}\",\"args\":["#,
            r#"\"qoder-cli\",\"hook\"],\"timeout\":5}]}]}}". {kr_hook} is replaced by the full path"#,
            r#" of the KalaReach forwarder installed on this machine, written as the path of a"#,
            r#" program the application starts. It sets no environment variables."#,
        ),
    },
];

impl ReviewedIntegration {
    /// Why `integration` is not this reviewed declaration, or nothing when it is.
    fn refusal(&self, integration: &CommandIntegration) -> Option<String> {
        if integration.command != self.command {
            return Some(format!(
                "the command is {:?}, not {:?}",
                integration.command, self.command
            ));
        }
        match self.flags {
            ReviewedFlags::Written(flags) => {
                if integration.flags != flags {
                    return Some(format!(
                        "the flags are {:?}, not {flags:?}",
                        integration.flags
                    ));
                }
            }
            ReviewedFlags::Pinned { count, sha256 } => {
                let mut written =
                    serde_json::to_string_pretty(&integration.flags).expect("a list of text");
                written.push('\n');
                let digest = PayloadDigest::of(written.as_bytes()).to_string();
                if integration.flags.len() != count || digest != sha256 {
                    return Some(format!(
                        "the {} flags are not the {count} pinned by {sha256}: their digest is \
                         {digest}",
                        integration.flags.len()
                    ));
                }
            }
        }
        let variables: Vec<(&str, &str)> = integration
            .variables
            .iter()
            .map(|variable| (variable.name.as_str(), variable.value.as_str()))
            .collect();
        if variables != self.variables {
            return Some(format!(
                "the variables are {variables:?}, not {:?}",
                self.variables
            ));
        }
        (integration.grant_statement.as_str() != self.grant_statement).then(|| {
            format!(
                "the grant is not the reviewed statement: {}",
                integration.grant_statement
            )
        })
    }

    /// Why `statement` is not the list reviewed for this integration, which the grant shows the
    /// owner beside its statement, or nothing when it is.
    ///
    /// The whole statement is compared with the reviewed list, so nothing is added, dropped or
    /// moved anywhere in it, in a string or between two. The reviewed list is then read back, so
    /// every string it writes is exactly the command, a flag or a variable's value of
    /// `integration`, whole and in its place.
    fn listed_refusal(&self, integration: &CommandIntegration, statement: &str) -> Option<String> {
        if statement != self.listed {
            return Some(format!(
                "the grant shows {statement:?}, not the reviewed list {:?}",
                self.listed
            ));
        }
        listing_refusal(integration, statement)
    }
}

/// Every JSON string `statement` writes, read whole and decoded, each with the text written between
/// it and the string before it.
///
/// The contract writes the command, each flag and each variable's value as a JSON string, and the
/// words around them hold no quote, so a quote outside a string opens one. A string ends at the
/// first quote no backslash escapes, and it is decoded as JSON: a quote escaped inside one string
/// never starts another, so no string is read as part of another.
fn written_strings(statement: &str) -> Result<Vec<(&str, String)>, String> {
    let bytes = statement.as_bytes();
    let mut strings = Vec::new();
    let mut words_start = 0;
    let mut at = 0;
    while at < bytes.len() {
        if bytes[at] != b'"' {
            at += 1;
            continue;
        }
        let mut end = at + 1;
        loop {
            match bytes.get(end) {
                None => return Err(format!("the string opened at byte {at} never ends")),
                Some(b'\\') => end += 2,
                Some(b'"') => break,
                Some(_) => end += 1,
            }
        }
        let value: String = serde_json::from_str(&statement[at..=end])
            .map_err(|error| format!("the string at byte {at} is not JSON: {error}"))?;
        strings.push((&statement[words_start..at], value));
        at = end + 1;
        words_start = at;
    }
    Ok(strings)
}

/// Why the strings `statement` writes are not exactly the command, the flags and the variables of
/// `integration`, in the order the host applies them, or nothing when they are.
///
/// Every string the statement writes, read whole, is exactly the command, then each flag, then each
/// variable's value, and no other string is written, so no string is shortened, added, dropped or
/// moved. A flag is never written after `=`, where it would read as a variable's value, and each
/// value is written after exactly its variable's own name. The other words are not this check's:
/// an argument written without quotes is no string, so [`ReviewedIntegration::listed_refusal`]
/// compares the whole statement with the reviewed list first.
fn listing_refusal(integration: &CommandIntegration, statement: &str) -> Option<String> {
    let strings = match written_strings(statement) {
        Ok(strings) => strings,
        Err(why) => return Some(format!("{why}: {statement}")),
    };
    let written: Vec<&str> = strings.iter().map(|(_, value)| value.as_str()).collect();
    let mut declared = vec![integration.command.as_str()];
    declared.extend(integration.flags.iter().map(String::as_str));
    declared.extend(
        integration
            .variables
            .iter()
            .map(|variable| variable.value.as_str()),
    );
    if written != declared {
        return Some(format!(
            "the statement writes {written:?}, not {declared:?}: {statement}"
        ));
    }
    let flags = &strings[1..=integration.flags.len()];
    if let Some((_, flag)) = flags.iter().find(|(words, _)| words.ends_with('=')) {
        return Some(format!(
            "the flag {flag:?} is written as a variable's value: {statement}"
        ));
    }
    let values = &strings[1 + integration.flags.len()..];
    for ((words, _), variable) in values.iter().zip(&integration.variables) {
        if !words.ends_with(&format!(" {}=", variable.name)) {
            return Some(format!(
                "the value of {} is not written after exactly its name: {statement}",
                variable.name
            ));
        }
    }
    None
}

/// Statements that each misstate `integration` in one way, made from the statement the host renders
/// for it: the last flag shortened, written inside another string, written as a variable's value or
/// followed by one more; the first two flags swapped; the first variable's value left out, written
/// under a longer name or followed by one more variable.
fn misstatements(integration: &CommandIntegration) -> Vec<String> {
    let rendered = integration.statement();
    let quoted = |text: &str| serde_json::to_string(text).expect("text is a JSON string");
    let mut wrong = Vec::new();
    if let Some(last) = integration.flags.last() {
        let written = quoted(last);
        let mut shorter = last.clone();
        shorter.pop();
        wrong.push(rendered.replacen(&written, &quoted(&shorter), 1));
        wrong.push(rendered.replacen(&written, &format!("\"prefix\\{written}"), 1));
        wrong.push(rendered.replacen(&written, &format!("EXTRA={written}"), 1));
        wrong.push(rendered.replacen(&written, &format!("{written} \"--extra\""), 1));
    }
    if let [first, second, ..] = integration.flags.as_slice() {
        wrong.push(rendered.replacen(
            &format!("{} {}", quoted(first), quoted(second)),
            &format!("{} {}", quoted(second), quoted(first)),
            1,
        ));
    }
    if let Some(variable) = integration.variables.first() {
        let written = format!("{}={}", variable.name, quoted(&variable.value));
        wrong.push(rendered.replacen(&written, &variable.name, 1));
        wrong.push(rendered.replacen(&written, &format!("NOT_{written}"), 1));
        wrong.push(rendered.replacen(&written, &format!("{written}, EXTRA=\"1\""), 1));
    }
    wrong
}

/// Statements that each add to what the host renders for `integration` in words no string holds:
/// an argument written without quotes between the first two flags or after the last one, or a
/// variable whose value is written without quotes after the last one.
fn unquoted_additions(integration: &CommandIntegration) -> Vec<String> {
    let rendered = integration.statement();
    let quoted = |text: &str| serde_json::to_string(text).expect("text is a JSON string");
    let mut added = Vec::new();
    if let [first, second, ..] = integration.flags.as_slice() {
        let pair = format!("{} {}", quoted(first), quoted(second));
        let spread = format!("{} --extra {}", quoted(first), quoted(second));
        added.push(rendered.replacen(&pair, &spread, 1));
    }
    if let Some(last) = integration.flags.last() {
        let written = format!("{}.", quoted(last));
        added.push(rendered.replacen(&written, &format!("{} --extra.", quoted(last)), 1));
    }
    if let Some(last) = integration.variables.last() {
        let written = format!("{}={}.", last.name, quoted(&last.value));
        let more = format!("{}={}, EXTRA=1.", last.name, quoted(&last.value));
        added.push(rendered.replacen(&written, &more, 1));
    }
    added
}

/// Every package that declares a command integration declares the reviewed one, validates, asks
/// for the capability that applies it and states the package contract that reads it, and the grant
/// a person confirms lists its command, every flag and every variable exactly. A reviewed package
/// that stops declaring its integration fails too.
#[test]
fn every_command_integration_is_the_reviewed_one_and_its_grant_lists_it_exactly() {
    let loaded = packages::load(&root()).expect("the repository loads");
    for package in &loaded.repository.packages {
        let manifest = &package.package.manifest;
        let plugin = manifest.plugin_id().to_string();
        assert!(
            manifest.command_integration.is_none()
                || REVIEWED_INTEGRATIONS
                    .iter()
                    .any(|reviewed| reviewed.plugin == plugin),
            "{plugin} declares a command integration nobody has reviewed"
        );
    }
    for reviewed in REVIEWED_INTEGRATIONS {
        let package = package_named(&loaded, reviewed.plugin);
        let manifest = &package.package.manifest;
        let integration = manifest
            .command_integration
            .as_ref()
            .unwrap_or_else(|| panic!("{} declares no command integration", reviewed.plugin));
        if let Some(refusal) = reviewed.refusal(integration) {
            panic!("{}: {refusal}", reviewed.plugin);
        }
        assert!(
            manifest.requests(PluginCapability::CommandIntegrationLaunch),
            "{} does not ask for the capability that applies its integration",
            reviewed.plugin
        );
        // A host on an earlier package contract cannot read the member, so it refuses the package
        // by its range rather than by a member it does not know.
        assert_eq!(
            manifest.sdk_range.to_string(),
            ">=0.1.3, <0.2.0",
            "{}",
            reviewed.plugin
        );
        if let Some(refusal) = reviewed.listed_refusal(integration, &integration.statement()) {
            panic!("{}: {refusal}", reviewed.plugin);
        }
    }
}

/// A reviewed declaration is compared whole: another command, a flag changed, added, dropped or
/// moved, a variable changed or added, or one word of the statement changed is not the reviewed
/// declaration, and each change to the command, a flag or a variable changes the list the grant
/// shows. For each declaration, the check that accepts the list the host renders refuses every
/// misstatement of its strings and every addition in words no string holds; and for a declaration
/// with two flags and a variable, the string check refuses every misstatement of its strings.
#[test]
fn a_changed_command_integration_is_not_the_reviewed_one() {
    let loaded = packages::load(&root()).expect("the repository loads");
    for reviewed in REVIEWED_INTEGRATIONS {
        let integration = package_named(&loaded, reviewed.plugin)
            .package
            .manifest
            .command_integration
            .clone()
            .unwrap_or_else(|| panic!("{} declares no command integration", reviewed.plugin));
        assert_eq!(reviewed.refusal(&integration), None);

        let mut changes: Vec<CommandIntegration> = Vec::new();
        let mut command = integration.clone();
        command.command.push('x');
        changes.push(command);
        let mut added = integration.clone();
        added.flags.push("--verbose".to_owned());
        changes.push(added);
        if let Some(last) = integration.flags.len().checked_sub(1) {
            let mut changed = integration.clone();
            changed.flags[last].push(' ');
            changes.push(changed);
            let mut dropped = integration.clone();
            dropped.flags.pop();
            changes.push(dropped);
        }
        if integration.flags.len() > 1 {
            let mut moved = integration.clone();
            moved.flags.swap(0, 1);
            changes.push(moved);
        }
        let mut variable = integration.clone();
        match variable.variables.first_mut() {
            Some(first) => first.value = "1".to_owned(),
            None => variable.variables.push(IntegrationVariable {
                name: "GEMINI_CLI_NO_RELAUNCH".to_owned(),
                value: "true".to_owned(),
            }),
        }
        changes.push(variable);
        let mut statement = integration.clone();
        statement.grant_statement = Summary::new(reviewed.grant_statement.replacen(
            "KalaReach",
            "Kala Reach",
            1,
        ))
        .expect("a statement");
        changes.push(statement);
        for (index, changed) in changes.iter().enumerate() {
            assert!(
                reviewed.refusal(changed).is_some(),
                "{}: {changed:?} is accepted as the reviewed declaration",
                reviewed.plugin
            );
            // Every change but the last, which is the grant's statement, changes the list.
            if index + 1 < changes.len() {
                assert!(
                    reviewed
                        .listed_refusal(changed, &changed.statement())
                        .is_some(),
                    "{}: {changed:?} shows the reviewed list",
                    reviewed.plugin
                );
            }
        }

        let rendered = integration.statement();
        assert_eq!(
            reviewed.listed_refusal(&integration, &rendered),
            None,
            "{rendered}"
        );
        let wrong: Vec<String> = misstatements(&integration)
            .into_iter()
            .chain(unquoted_additions(&integration))
            .collect();
        assert!(!wrong.is_empty(), "{rendered}");
        for statement in wrong {
            assert_ne!(statement, rendered, "a misstatement that changes nothing");
            assert!(
                reviewed.listed_refusal(&integration, &statement).is_some(),
                "{}: {statement} is accepted as the reviewed list",
                reviewed.plugin
            );
        }
    }
    assert_statement_is_checked(&CommandIntegration {
        command: "agent".to_owned(),
        flags: vec!["--one".to_owned(), "--two".to_owned()],
        variables: vec![IntegrationVariable {
            name: "GEMINI_CLI_NO_RELAUNCH".to_owned(),
            value: "true".to_owned(),
        }],
        grant_statement: Summary::new("A statement").expect("a statement"),
    });
}

/// The string check accepts the statement the host renders for `integration` and refuses every
/// misstatement of its strings.
fn assert_statement_is_checked(integration: &CommandIntegration) {
    let rendered = integration.statement();
    assert_eq!(listing_refusal(integration, &rendered), None, "{rendered}");
    let wrong = misstatements(integration);
    assert!(!wrong.is_empty(), "{rendered}");
    for statement in wrong {
        assert_ne!(statement, rendered, "a misstatement that changes nothing");
        assert!(
            listing_refusal(integration, &statement).is_some(),
            "{statement} is accepted as {rendered}"
        );
    }
}

/// A package whose command integration sets a variable outside the contract's closed list is
/// refused, where the same package with its reviewed declaration loads, and the refusal is the
/// variable's and nothing else's. A permitted name with another value, a variable that loads code
/// into the process and a name reserved for the worker are each outside the list.
#[test]
fn a_command_integration_that_sets_a_variable_outside_the_closed_list_is_refused() {
    for (name, value) in [
        ("GEMINI_CLI_NO_RELAUNCH", "1"),
        ("NODE_OPTIONS", "--require /tmp/preload.js"),
        ("KR_REGISTRATION", "/tmp/registration"),
    ] {
        let temporary = tempfile::tempdir().expect("a temporary directory");
        let repository = temporary.path();
        let package = repository_holding(repository, "gemini-cli");
        let loaded = packages::load(repository).expect("the copy loads");
        assert!(
            loaded.rejected.is_empty(),
            "the copy as it is: {:?}",
            loaded
                .rejected
                .iter()
                .map(|rejected| &rejected.report.findings)
                .collect::<Vec<_>>()
        );

        let path = package.join("plugin.json");
        let mut manifest: serde_json::Value =
            kalareach_catalogue::read_json(&path).expect("the manifest reads");
        let variables = manifest
            .get_mut("command_integration")
            .and_then(|integration| integration.get_mut("variables"))
            .expect("gemini-cli declares the variables its integration sets");
        *variables = serde_json::json!([{ "name": name, "value": value }]);
        let mut text = serde_json::to_string_pretty(&manifest).expect("the manifest renders");
        text.push('\n');
        std::fs::write(&path, text).expect("the manifest writes");

        let loaded = packages::load(repository).expect("the repository loads");
        let [rejected] = loaded.rejected.as_slice() else {
            panic!(
                "{name}={value}: {} packages are refused, not one",
                loaded.rejected.len()
            );
        };
        assert_eq!(
            rejected.report.codes(),
            [FindingCode::IntegrationInvalid],
            "{name}={value}: {:?}",
            rejected.report.findings
        );
    }
}

/// A repository in `repository` holding the publisher record and a copy of one of this
/// repository's packages, and the copy's directory.
fn repository_holding(repository: &Path, plugin: &str) -> PathBuf {
    std::fs::create_dir_all(repository.join("publishers")).expect("publishers directory");
    std::fs::copy(
        root().join("publishers/kalareach.json"),
        repository.join("publishers/kalareach.json"),
    )
    .expect("the publisher record copies");
    let destination = repository.join("plugins/kalareach").join(plugin);
    copy_tree(&root().join("plugins/kalareach").join(plugin), &destination);
    destination
}

/// Every hook registration in a Claude Code hooks file, with the event it is registered for.
fn hook_registrations(file: &serde_json::Value) -> Vec<(&str, &serde_json::Value)> {
    let events = file
        .get("hooks")
        .and_then(serde_json::Value::as_object)
        .expect("the file registers hooks by event");
    let mut registrations = Vec::new();
    for (event, groups) in events {
        for group in groups.as_array().expect("each event lists its groups") {
            let handlers = group
                .get("hooks")
                .and_then(serde_json::Value::as_array)
                .expect("each group lists its hooks");
            for handler in handlers {
                registrations.push((event.as_str(), handler));
            }
        }
    }
    registrations
}

/// What a registration names the forwarder by: the host writes the installed forwarder's full path
/// in its place when it installs the registration.
const FORWARDER_PLACEHOLDER: &str = "{kr_hook}";

/// Why one hook registration is not the forwarder's hook for `application` started in exec form,
/// or nothing when it is.
///
/// Exec form is a `command` that names a program and an `args` list, which Claude Code and Qoder
/// CLI start with no shell between them. The worker lets only a hook the application started itself
/// select the thread, so a shell string, or a shell that starts the forwarder, leaves every report
/// moving nothing. A member this check does not know could change how the hook runs, in the
/// background for one, so a registration carries only its type, its command, its arguments and a
/// timeout.
fn hook_registration_refusal(application: &str, handler: &serde_json::Value) -> Option<String> {
    let Some(members) = handler.as_object() else {
        return Some("the registration is not an object".to_owned());
    };
    if let Some(other) = members
        .keys()
        .find(|name| !matches!(name.as_str(), "type" | "command" | "args" | "timeout"))
    {
        return Some(format!("the registration carries {other}"));
    }
    if handler.get("type") != Some(&serde_json::json!("command")) {
        return Some("the registration is not a command".to_owned());
    }
    let Some(command) = handler.get("command").and_then(serde_json::Value::as_str) else {
        return Some("the registration names no command".to_owned());
    };
    if command != FORWARDER_PLACEHOLDER
        && (command.is_empty()
            || command.chars().any(|character| {
                character.is_whitespace() || "\"'`$;&|<>(){}[]*?~#!\\".contains(character)
            }))
    {
        return Some(format!("the command {command:?} is a shell string"));
    }
    let Some(args) = handler.get("args").and_then(serde_json::Value::as_array) else {
        return Some("the registration has no argument list".to_owned());
    };
    let Some(args) = args
        .iter()
        .map(serde_json::Value::as_str)
        .collect::<Option<Vec<&str>>>()
    else {
        return Some("an argument is not text".to_owned());
    };
    if command != FORWARDER_PLACEHOLDER || args != [application, "hook"] {
        return Some(format!(
            "the registration starts {command} {args:?}, not the forwarder's hook for {application}"
        ));
    }
    None
}

/// Why a Claude Code hooks file registers a hook other than the forwarder in exec form.
fn hooks_file_refusal(file: &serde_json::Value) -> Option<String> {
    hook_registrations(file)
        .into_iter()
        .find_map(|(event, handler)| {
            hook_registration_refusal("claude-code", handler)
                .map(|refusal| format!("{event}: {refusal}"))
        })
}

/// Every hook the Claude Code bridge registers starts the forwarder in exec form, and the same
/// check refuses the file once any registration in it is moved to a shell form.
#[test]
fn every_claude_code_hook_starts_the_forwarder_in_exec_form() {
    let loaded = packages::load(&root()).expect("the repository loads");
    let package = package_named(&loaded, "kalareach/claude-code");
    let file: serde_json::Value =
        kalareach_catalogue::read_json(&package.directory.join("bridge/hooks.json"))
            .expect("the hooks file reads");
    assert_eq!(
        hook_registrations(&file).len(),
        5,
        "one registration for each of the five events"
    );
    assert_eq!(hooks_file_refusal(&file), None);

    let exec = hook_registrations(&file)[0].1.clone();
    let mut backgrounded = exec.clone();
    backgrounded["async"] = serde_json::json!(true);
    let shell_forms = [
        serde_json::json!({ "type": "command", "command": "kr-hook claude-code hook", "timeout": 5 }),
        serde_json::json!({
            "type": "command", "command": "kr-hook claude-code hook", "args": [], "timeout": 5
        }),
        serde_json::json!({
            "type": "command", "command": "/bin/sh", "args": ["-c", "kr-hook claude-code hook"],
            "timeout": 5
        }),
        backgrounded,
    ];
    let events: Vec<String> = hook_registrations(&file)
        .iter()
        .map(|(event, _)| (*event).to_owned())
        .collect();
    for event in &events {
        for shell_form in &shell_forms {
            let mut copy = file.clone();
            copy["hooks"][event.as_str()][0]["hooks"][0] = shell_form.clone();
            assert!(
                hooks_file_refusal(&copy).is_some(),
                "{event}: a registration moved to {shell_form} is accepted"
            );
        }
    }
}

/// The events the settings of a Qoder CLI launch register the forwarder for, each with its timeout
/// in seconds. On these five only exit code 2 refuses anything, and the forwarder never exits 2.
const QODER_CLI_HOOKS: &[(&str, u64)] = &[
    ("Notification", 5),
    ("PostToolUse", 5),
    ("PostToolUseFailure", 5),
    ("SessionEnd", 1),
    ("SessionStart", 5),
];

/// Why the settings a Qoder CLI launch passes register anything but the forwarder's hook in exec
/// form, with no matcher, for exactly its five events, or nothing when they do not.
fn qoder_settings_refusal(settings: &serde_json::Value) -> Option<String> {
    let Some(members) = settings.as_object() else {
        return Some("the settings are not an object".to_owned());
    };
    if members.len() != 1 {
        return Some("the settings carry more than hooks".to_owned());
    }
    let Some(events) = settings.get("hooks").and_then(serde_json::Value::as_object) else {
        return Some("the settings register no hooks by event".to_owned());
    };
    let mut registered: Vec<&str> = events.keys().map(String::as_str).collect();
    registered.sort_unstable();
    let expected: Vec<&str> = QODER_CLI_HOOKS.iter().map(|(event, _)| *event).collect();
    if registered != expected {
        return Some(format!(
            "the settings register {registered:?}, not {expected:?}"
        ));
    }
    for (event, timeout) in QODER_CLI_HOOKS {
        let Some(groups) = events[*event].as_array() else {
            return Some(format!("{event}: no list of groups"));
        };
        let [group] = groups.as_slice() else {
            return Some(format!("{event}: one group, not {}", groups.len()));
        };
        if group.as_object().map(serde_json::Map::len) != Some(1) {
            return Some(format!(
                "{event}: the group carries more than its hooks, a matcher for one"
            ));
        }
        let Some(handlers) = group.get("hooks").and_then(serde_json::Value::as_array) else {
            return Some(format!("{event}: the group lists no hooks"));
        };
        let [handler] = handlers.as_slice() else {
            return Some(format!("{event}: one hook, not {}", handlers.len()));
        };
        if let Some(refusal) = hook_registration_refusal("qoder-cli", handler) {
            return Some(format!("{event}: {refusal}"));
        }
        if handler.get("timeout") != Some(&serde_json::json!(timeout)) {
            return Some(format!("{event}: the timeout is not {timeout} seconds"));
        }
    }
    None
}

/// Qoder CLI's command integration adds `--settings` and settings that hold hooks and nothing else:
/// the forwarder's hook in exec form for each of its five events, with no matcher, so Qoder CLI
/// starts the forwarder itself for every tool, notification and session source. The same check
/// refuses the settings once a registration moves to a shell form or names another application,
/// once a group gains a matcher, once an event is dropped, or once the settings carry anything but
/// hooks.
#[test]
fn qoder_cli_s_integration_passes_the_forwarder_s_hooks_in_exec_form_and_nothing_else() {
    let loaded = packages::load(&root()).expect("the repository loads");
    let integration = package_named(&loaded, "kalareach/qoder-cli")
        .package
        .manifest
        .command_integration
        .as_ref()
        .expect("qoder-cli declares its command integration");
    let [flag, inline] = integration.flags.as_slice() else {
        panic!("two flags, not {}", integration.flags.len());
    };
    assert_eq!(flag, "--settings");
    let settings: serde_json::Value = serde_json::from_str(inline).expect("the settings are JSON");
    assert_eq!(qoder_settings_refusal(&settings), None);

    let mut shell = settings.clone();
    shell["hooks"]["SessionStart"][0]["hooks"][0] = serde_json::json!({
        "type": "command", "command": "kr-hook qoder-cli hook", "timeout": 5
    });
    let mut other = settings.clone();
    other["hooks"]["SessionEnd"][0]["hooks"][0]["args"] =
        serde_json::json!(["claude-code", "hook"]);
    let mut matcher = settings.clone();
    matcher["hooks"]["PostToolUse"][0]["matcher"] = serde_json::json!("Bash");
    let mut dropped = settings.clone();
    dropped["hooks"]
        .as_object_mut()
        .expect("hooks by event")
        .remove("Notification");
    let mut more = settings.clone();
    more["permissions"] = serde_json::json!({ "allow": ["Bash"] });
    for changed in [shell, other, matcher, dropped, more] {
        assert!(
            qoder_settings_refusal(&changed).is_some(),
            "{changed} is accepted"
        );
    }
}

/// The events the Gemini CLI extension registers, each with its timeout in milliseconds.
///
/// Gemini CLI ignores a refusal on these three, and on nothing else it runs a hook for: it reads a
/// hook's standard error as its answer when standard output is empty and turns plain text with an
/// exit code other than 0 and 1 into a refusal, which on a finished tool replaces the result the
/// model reads. So the extension registers these and no tool event.
const GEMINI_CLI_HOOKS: &[(&str, u64)] = &[
    ("Notification", 5000),
    ("SessionEnd", 1000),
    ("SessionStart", 5000),
];

/// Why one Gemini CLI hook registration is not the forwarder as a command of plain words, or
/// nothing when it is.
///
/// Gemini CLI runs a hook's command with `bash -c`. A command of plain words is one bash runs in its
/// own process, so the forwarder is Gemini CLI's own child with no shell left between them; any
/// shell syntax keeps a shell there, and could change what the hook prints. A member this check does
/// not know could change how the hook runs, its environment for one, so a registration carries only
/// its type, its name, its command and a timeout.
fn gemini_hook_registration_refusal(handler: &serde_json::Value) -> Option<String> {
    let Some(members) = handler.as_object() else {
        return Some("the registration is not an object".to_owned());
    };
    if let Some(other) = members
        .keys()
        .find(|name| !matches!(name.as_str(), "type" | "name" | "command" | "timeout"))
    {
        return Some(format!("the registration carries {other}"));
    }
    if handler.get("type") != Some(&serde_json::json!("command")) {
        return Some("the registration is not a command".to_owned());
    }
    if handler.get("name") != Some(&serde_json::json!("kalareach")) {
        return Some("the registration is not named kalareach".to_owned());
    }
    let Some(command) = handler.get("command").and_then(serde_json::Value::as_str) else {
        return Some("the registration names no command".to_owned());
    };
    // The words after the placeholder are the shell's to read, so they are plain; the placeholder is
    // the forwarder's full path, written in by the host and quoted for the shell.
    let words = command
        .strip_prefix(&format!("{FORWARDER_PLACEHOLDER} "))
        .unwrap_or(command);
    let plain = words.split(' ').all(|word| {
        !word.is_empty()
            && word
                .chars()
                .all(|character| character.is_ascii_alphanumeric() || character == '-')
    });
    if !plain {
        return Some(format!("the command {command:?} is not plain words"));
    }
    if command != format!("{FORWARDER_PLACEHOLDER} gemini-cli hook") {
        return Some(format!(
            "the registration starts {command:?}, not the forwarder's hook"
        ));
    }
    if handler
        .get("timeout")
        .and_then(serde_json::Value::as_u64)
        .is_none()
    {
        return Some("the registration has no timeout in milliseconds".to_owned());
    }
    None
}

/// Why a Gemini CLI hooks file registers anything but the forwarder's hook for its three events.
fn gemini_hooks_file_refusal(file: &serde_json::Value) -> Option<String> {
    let Some(file_members) = file.as_object() else {
        return Some("the file is not an object".to_owned());
    };
    if file_members.len() != 1 {
        return Some("the file carries more than its hooks".to_owned());
    }
    let Some(events) = file.get("hooks").and_then(serde_json::Value::as_object) else {
        return Some("the file registers no hooks by event".to_owned());
    };
    let mut registered: Vec<&str> = events.keys().map(String::as_str).collect();
    registered.sort_unstable();
    let expected: Vec<&str> = GEMINI_CLI_HOOKS.iter().map(|(event, _)| *event).collect();
    if registered != expected {
        return Some(format!(
            "the file registers {registered:?}, not {expected:?}"
        ));
    }
    for (event, timeout) in GEMINI_CLI_HOOKS {
        let Some(groups) = events[*event].as_array() else {
            return Some(format!("{event}: no list of groups"));
        };
        let [group] = groups.as_slice() else {
            return Some(format!("{event}: one group, not {}", groups.len()));
        };
        if group.as_object().map(serde_json::Map::len) != Some(1) {
            return Some(format!(
                "{event}: the group carries more than its hooks, a matcher or an order"
            ));
        }
        let Some(handlers) = group.get("hooks").and_then(serde_json::Value::as_array) else {
            return Some(format!("{event}: the group lists no hooks"));
        };
        let [handler] = handlers.as_slice() else {
            return Some(format!("{event}: one hook, not {}", handlers.len()));
        };
        if let Some(refusal) = gemini_hook_registration_refusal(handler) {
            return Some(format!("{event}: {refusal}"));
        }
        if handler.get("timeout") != Some(&serde_json::json!(timeout)) {
            return Some(format!(
                "{event}: the timeout is not {timeout} milliseconds"
            ));
        }
    }
    None
}

/// Every hook the Gemini CLI extension registers starts the forwarder as a command of plain words,
/// for exactly the three events whose refusals Gemini CLI ignores, and the same check refuses the
/// file once any registration in it gains shell syntax, another program, another member or a
/// matcher, or once a tool event is added. The extension's manifest names the directory the recipe
/// installs all three files into, and nothing else it could load.
///
/// The third file is Gemini CLI's install record. Where a person's settings list allowed extensions,
/// Gemini CLI refuses to start while any extension directory lacks one, and loads an extension only
/// when a listed pattern matches the source its record names. It also reads a local extension's
/// updates from that source, resolving a relative one, `~/...` included, from the session's working
/// directory, where a project could put a newer manifest. So the record names `/dev/null/kalareach`,
/// an absolute path nothing can exist under, and nothing else. Each install step's source,
/// destination and digest are checked together, the record's first, and removal deletes each file
/// with its installed digest in reverse, the record last: a recipe stopped part way never leaves the
/// manifest without it.
#[test]
fn every_gemini_cli_hook_starts_the_forwarder_as_plain_words() {
    let loaded = packages::load(&root()).expect("the repository loads");
    let package = package_named(&loaded, "kalareach/gemini-cli");
    let file: serde_json::Value =
        kalareach_catalogue::read_json(&package.directory.join("bridge/hooks.json"))
            .expect("the hooks file reads");
    assert_eq!(gemini_hooks_file_refusal(&file), None);

    let shell_forms = [
        serde_json::json!("kr-hook gemini-cli hook; true"),
        serde_json::json!("kr-hook gemini-cli hook 2>/dev/null"),
        serde_json::json!("kr-hook gemini-cli hook || echo {}"),
        serde_json::json!("$HOME/bin/kr-hook gemini-cli hook"),
        serde_json::json!("sh -c 'kr-hook gemini-cli hook'"),
        serde_json::json!("kr-hook  gemini-cli hook"),
        serde_json::json!("kr-hook claude-code hook"),
    ];
    for (event, _) in GEMINI_CLI_HOOKS {
        for shell_form in &shell_forms {
            let mut copy = file.clone();
            copy["hooks"][*event][0]["hooks"][0]["command"] = shell_form.clone();
            assert!(
                gemini_hooks_file_refusal(&copy).is_some(),
                "{event}: a registration moved to {shell_form} is accepted"
            );
        }
        for (member, value) in [
            ("env", serde_json::json!({"KR_REGISTRATION": "/tmp/r"})),
            ("description", serde_json::json!("more")),
        ] {
            let mut copy = file.clone();
            copy["hooks"][*event][0]["hooks"][0][member] = value;
            assert!(
                gemini_hooks_file_refusal(&copy).is_some(),
                "{event}: a registration carrying {member} is accepted"
            );
        }
        let mut matched = file.clone();
        matched["hooks"][*event][0]["matcher"] = serde_json::json!("startup");
        assert!(
            gemini_hooks_file_refusal(&matched).is_some(),
            "{event}: a matcher is accepted"
        );
    }
    let mut tool = file.clone();
    tool["hooks"]["AfterTool"] = file["hooks"]["SessionStart"].clone();
    assert!(
        gemini_hooks_file_refusal(&tool).is_some(),
        "a tool event is accepted"
    );

    let manifest: serde_json::Value =
        kalareach_catalogue::read_json(&package.directory.join("bridge/gemini-extension.json"))
            .expect("the extension manifest reads");
    let mut members: Vec<&str> = manifest
        .as_object()
        .expect("the manifest is an object")
        .keys()
        .map(String::as_str)
        .collect();
    members.sort_unstable();
    assert_eq!(members, ["description", "name", "version"]);
    let name = manifest["name"].as_str().expect("the extension's name");
    let bridge = package
        .package
        .manifest
        .native_bridge
        .0
        .as_ref()
        .expect("the package declares its bridge");
    let installs: Vec<(String, String, String)> = bridge
        .install
        .iter()
        .map(|step| match step {
            kr_plugin_sdk::plugin::BridgeStep::InstallFile {
                source,
                destination,
                digest,
            } => (
                source.as_str().to_owned(),
                destination.as_str().to_owned(),
                digest.to_string(),
            ),
            other => panic!("the recipe edits a configuration file: {other:?}"),
        })
        .collect();
    let placed: Vec<(String, String)> = installs
        .iter()
        .map(|(source, destination, _)| (source.clone(), destination.clone()))
        .collect();
    let expected = [
        (
            "bridge/gemini-extension-install.json",
            ".gemini-extension-install.json",
        ),
        ("bridge/gemini-extension.json", "gemini-extension.json"),
        ("bridge/hooks.json", "hooks/hooks.json"),
    ]
    .map(|(source, file)| (source.to_owned(), format!("extensions/{name}/{file}")));
    assert_eq!(
        placed, expected,
        "each file goes where Gemini CLI reads it from, in the directory the manifest names, the record first"
    );
    let removals: Vec<(String, String)> = bridge
        .remove
        .iter()
        .map(|step| match step {
            kr_plugin_sdk::plugin::BridgeRemoval::RemoveFile {
                destination,
                digest,
            } => (destination.as_str().to_owned(), digest.to_string()),
            other => panic!("the removal edits a configuration file: {other:?}"),
        })
        .rev()
        .collect();
    let installed: Vec<(String, String)> = installs
        .iter()
        .map(|(_, destination, digest)| (destination.clone(), digest.clone()))
        .collect();
    assert_eq!(
        removals, installed,
        "removal deletes each installed file with its installed digest, in reverse, so the record goes last"
    );

    let record: serde_json::Value = kalareach_catalogue::read_json(
        &package
            .directory
            .join("bridge/gemini-extension-install.json"),
    )
    .expect("the install record reads");
    assert_eq!(
        record,
        serde_json::json!({"source": "/dev/null/kalareach", "type": "local"}),
        "the install record names a local source nothing can exist under"
    );
}

/// Whether a visibility predicate is false whenever no approval is pending.
///
/// The `pending_approval` flag holds exactly when the host holds a pending approval resource, and
/// only a native request makes one; a term that names one request holds only while that request
/// is pending. `all` needs one such term and `any` needs every term to be one. A term under `not`
/// proves nothing, so it is not counted.
fn requires_pending_approval(predicate: &Predicate) -> bool {
    match predicate {
        Predicate::Flag {
            flag: PresentationFlag::PendingApproval,
        }
        | Predicate::PendingApprovalFor { .. } => true,
        Predicate::All { terms } => terms.iter().any(requires_pending_approval),
        Predicate::Any { terms } => {
            !terms.is_empty() && terms.iter().all(requires_pending_approval)
        }
        _ => false,
    }
}

/// Why a control that answers an approval can be used with no native request pending, or nothing
/// when it cannot. A control is usable only while both of its predicates hold, so one of them has
/// to require a pending approval.
fn approval_control_refusal(control: &Control) -> Option<String> {
    (!requires_pending_approval(&control.visible_when)
        && !requires_pending_approval(&control.enabled_when))
    .then(|| {
        format!(
            "the control {} answers an approval and can be used with none pending",
            control.id
        )
    })
}

/// Every bundled adapter says what it observes and what it controls, and a control that answers an
/// approval is usable only while a native request is pending.
///
/// Section 1: an adapter describes what it can observe and what it can control, and a convincing
/// reconstruction of terminal text does not establish authority to approve an operation. Each
/// profile of each bundled agent asks to read the broker's semantic events, requests the
/// capability behind every action it declares, and gives every method its table routes a class.
/// Its controls that answer an approval depend on the host's pending-approval fact, which only a
/// native request sets, so nothing a screen shows can make one usable. The check refuses a control
/// that shows while a person is awaited, on a negated flag, on the flag as one choice of two, or
/// always.
#[test]
fn every_bundled_adapter_declares_what_it_observes_and_controls_and_answers_only_a_native_request()
{
    let loaded = packages::load(&root()).expect("the repository loads");
    assert_eq!(BUNDLED_AGENTS.len(), 6, "section 12 bundles six agents");
    let mut approval_controls = 0;
    for (agent, plugins) in BUNDLED_AGENTS {
        for plugin in *plugins {
            let package = package_named(&loaded, plugin);
            let manifest = &package.package.manifest;
            let requested: Vec<PluginCapability> = manifest
                .capabilities
                .iter()
                .map(|request| request.capability)
                .collect();
            assert!(
                requested.contains(&PluginCapability::BrokerSemanticEvents),
                "{agent} ({plugin}) declares nothing it observes"
            );
            for action in &manifest.actions {
                let needed = action.effect.required_capability();
                assert!(
                    requested.contains(&needed),
                    "{plugin}: the action {} needs {needed}, which the package does not request",
                    action.id
                );
            }
            if let Some(connector) = &package.package.connector {
                for route in &connector.routes {
                    assert!(
                        connector
                            .methods
                            .iter()
                            .any(|entry| entry.method == route.method),
                        "{plugin}: {} is routed and has no class",
                        route.method
                    );
                }
            }
            let answering: Vec<_> = manifest
                .actions
                .iter()
                .filter(|action| action.effect == EffectClass::ApprovalRespond)
                .map(|action| &action.id)
                .collect();
            for control in package
                .package
                .presentation
                .nodes
                .iter()
                .flat_map(|node| node.body.controls())
                .filter(|control| answering.contains(&&control.action_id))
            {
                approval_controls += 1;
                if let Some(refusal) = approval_control_refusal(control) {
                    panic!("{plugin}: {refusal}");
                }
            }
        }
    }
    assert!(
        approval_controls > 0,
        "no bundled adapter draws a control that answers an approval"
    );

    let claude = package_named(&loaded, "kalareach/claude-code");
    let allow = claude
        .package
        .presentation
        .nodes
        .iter()
        .flat_map(|node| node.body.controls())
        .find(|control| control.id.as_str() == "allow")
        .expect("the Claude Code document draws Allow");
    let grant = Predicate::Grant {
        right: ActionRight::AgentApprovalRespond,
    };
    let flag = Predicate::Flag {
        flag: PresentationFlag::PendingApproval,
    };
    for (when, visible) in [
        (
            "a person is awaited",
            Predicate::All {
                terms: vec![
                    grant.clone(),
                    Predicate::Binding {
                        state: BindingState::AwaitingPerson,
                    },
                ],
            },
        ),
        (
            "the flag is negated",
            Predicate::All {
                terms: vec![
                    grant.clone(),
                    Predicate::Not {
                        term: Box::new(flag.clone()),
                    },
                ],
            },
        ),
        (
            "the flag is one choice of two",
            Predicate::Any {
                terms: vec![grant.clone(), flag.clone()],
            },
        ),
        ("always", Predicate::Always {}),
    ] {
        let mut control = allow.clone();
        control.visible_when = visible;
        assert!(
            approval_control_refusal(&control).is_some(),
            "a control that answers an approval and shows when {when} is accepted"
        );
    }
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
        for (file_stem, path_suffix) in entry.required_rules {
            assert!(
                manifest.match_rules.iter().any(|rule| {
                    rule.executable.file_stem == *file_stem
                        && rule.executable.path_suffix == *path_suffix
                }),
                "{plugin} carries no rule for {file_stem} under {}",
                path_suffix.join("/")
            );
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

/// The package the repository publishes under one identifier.
fn package_named<'a>(loaded: &'a packages::Loaded, plugin: &str) -> &'a packages::LoadedPackage {
    loaded
        .repository
        .packages
        .iter()
        .find(|package| package.package.manifest.plugin_id().as_str() == plugin)
        .unwrap_or_else(|| panic!("{plugin} is published by this repository"))
}

/// The frame a corpus pins for one situation.
fn frame_in<'a>(corpus: &'a FrameCorpus, situation: &str) -> &'a serde_json::Value {
    let mut found = corpus
        .frames
        .iter()
        .filter(|case| case.situation == situation);
    let frame = found
        .next()
        .unwrap_or_else(|| panic!("the corpus pins no frame for {situation}"));
    assert!(
        found.next().is_none(),
        "the corpus pins two frames for {situation}"
    );
    &frame.frame
}

fn decision(name: &str) -> ParameterName {
    ParameterName::new(name).expect("a valid decision name")
}

/// A relayed Claude Code approval is answered from the table, for the request it relayed and for
/// nothing else.
///
/// The table names the request it answers, the method that carries the answer and the value each
/// decision becomes. What it writes for the pinned request has to be the pinned answer exactly:
/// that request's own identifier, never another's, and the vendor's own value. No other message on
/// this surface is a request it answers, including an answer and a message into the session, so none
/// of them gets an answer however its text reads.
#[test]
fn a_relayed_claude_code_approval_is_answered_with_its_own_identifier_and_nothing_else_is() {
    let loaded = packages::load(&root()).expect("the repository loads");
    let package = package_named(&loaded, "kalareach/claude-code");
    let connector = package
        .package
        .connector
        .as_ref()
        .expect("the package carries a connector table");
    let corpus: FrameCorpus =
        kalareach_catalogue::read_json(&package.directory.join("fixtures/frames.json"))
            .expect("the corpus reads");

    let request = frame_in(&corpus, "a relayed tool approval");
    let method = at(request, &connector.method_path)
        .and_then(serde_json::Value::as_str)
        .expect("the request names its method");
    let identifier =
        at(request, &connector.request_id_path).expect("the request carries its identifier");

    let allowed = connector
        .answer(method, identifier, &decision("allow"))
        .expect("the table answers the request it relays");
    assert_eq!(
        &allowed,
        frame_in(&corpus, "the answer to that approval"),
        "the table does not write the pinned answer for the pinned request"
    );
    assert_eq!(at(&allowed, &connector.request_id_path), Some(identifier));
    assert_ne!(
        &allowed,
        frame_in(&corpus, "an answer naming a request nobody issued"),
        "an answer names a request other than the one it answers"
    );

    let denied = connector
        .answer(method, identifier, &decision("deny"))
        .expect("the table answers with a refusal too");
    assert_eq!(
        member(&denied, "params.behavior"),
        Some(&serde_json::json!("deny"))
    );
    assert_eq!(at(&denied, &connector.request_id_path), Some(identifier));

    // A decision the table does not map is refused rather than sent under some nearby value.
    assert!(matches!(
        connector.answer(method, identifier, &decision("always")),
        Err(AnswerError::UnknownDecision { .. })
    ));

    // Every other message the corpus pins is one the table does not answer.
    let mut others = 0;
    for case in &corpus.frames {
        let wire = at(&case.frame, &connector.method_path)
            .and_then(serde_json::Value::as_str)
            .expect("every pinned frame names its method");
        if wire == method {
            continue;
        }
        others += 1;
        assert!(
            matches!(
                connector.answer(wire, identifier, &decision("allow")),
                Err(AnswerError::NotAnswered { .. })
            ),
            "[{}]: the table answers a message that is not the request it relays",
            case.situation
        );
    }
    assert!(others > 0, "the corpus pins no message but the request");
}

/// An answer to a Claude Code approval names the pending request it answers.
///
/// The request a person answers is the one the call names, and the host checks that name again
/// when the call arrives. A call that names none is refused, so nothing answers whichever request
/// happens to be waiting. Every decision the action offers is one the table maps.
#[test]
fn an_answer_to_a_claude_code_approval_names_the_pending_request_it_answers() {
    let loaded = packages::load(&root()).expect("the repository loads");
    let package = package_named(&loaded, "kalareach/claude-code");
    let manifest = &package.package.manifest;
    let destination = package
        .package
        .connector
        .as_ref()
        .and_then(|connector| connector.decision_destination.as_ref())
        .expect("the table declares where an answer goes");

    let answering: Vec<_> = manifest
        .actions
        .iter()
        .filter(|action| action.effect == EffectClass::ApprovalRespond)
        .collect();
    let [action] = answering.as_slice() else {
        panic!(
            "the package declares {} answering actions, not one",
            answering.len()
        );
    };
    let ActionImplementation::DecisionDestination {
        decision: parameter,
    } = &action.implementation
    else {
        panic!(
            "{} answers some other way than through the table",
            action.id
        );
    };
    let offered: Vec<&ParameterName> = action
        .parameters
        .parameters
        .iter()
        .filter(|declared| &declared.name == parameter)
        .flat_map(|declared| match &declared.kind {
            ParameterKind::Choice { choices } => choices.iter().map(|choice| &choice.id).collect(),
            _ => Vec::new(),
        })
        .collect();
    assert!(!offered.is_empty(), "{} offers no decision", action.id);

    let call = |resource: serde_json::Value, choice: &str| -> ActionInvocation {
        serde_json::from_value(serde_json::json!({
            "action_id": action.id,
            "resource_id": resource,
            "arguments": [
                { "name": parameter, "value": { "type": "choice", "choice_id": choice } }
            ],
        }))
        .expect("a well-formed invocation")
    };
    let pending = serde_json::json!("5f8e2c1a-4b3d-4e6f-8a9b-0c1d2e3f4a5b");

    for choice in &offered {
        assert!(
            matches!(
                action.check(&call(serde_json::Value::Null, choice.as_str())),
                Err(InvocationError::ResourceMissing { .. })
            ),
            "an answer that names no pending request is not refused"
        );
        let named = call(pending.clone(), choice.as_str());
        assert_eq!(action.check(&named), Ok(()));
        assert_eq!(action.decision(&named), Some(*choice));
        assert!(
            destination.value_for(choice).is_some(),
            "the table maps no value for {choice}"
        );
    }
    assert!(matches!(
        action.check(&call(pending, "always")),
        Err(InvocationError::Arguments(_))
    ));
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
/// shared-server family publishes none the earlier one lacks. So the earlier family's package has
/// no name it could refuse, and what tells the families apart there is its own exclusive name: its
/// table reads it as the event it is, and the shared-server package refuses the same name outright.
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

/// The two Kimi distributions, each by the package that carries its profile.
const KIMI_DISTRIBUTIONS: [&str; 2] = ["kalareach/kimi-cli", "kalareach/kimi-code-cli"];

/// The session capabilities an agent-protocol handshake advertises that each name one method.
///
/// `additionalDirectories` is advertised too, and it widens what `session/new` accepts rather than
/// naming a method of its own, so it is not here.
const SESSION_OPERATIONS: &[(&str, &str)] = &[
    ("close", "session/close"),
    ("delete", "session/delete"),
    ("fork", "session/fork"),
    ("list", "session/list"),
    ("resume", "session/resume"),
];

/// The handshake answer a package pins under `evidence.handshake` in its frame corpus.
#[derive(serde::Deserialize)]
struct PinnedHandshake {
    #[serde(rename = "agentInfo")]
    agent_info: PinnedAgent,
    #[serde(rename = "sessionCapabilities")]
    session_capabilities: BTreeSet<String>,
}

#[derive(serde::Deserialize)]
struct PinnedAgent {
    name: String,
    version: String,
}

#[test]
fn the_kimi_distributions_share_an_agent_name_and_differ_in_what_their_handshake_advertises() {
    let loaded = packages::load(&root()).expect("the repository loads");
    let pinned: Vec<_> = KIMI_DISTRIBUTIONS
        .iter()
        .map(|plugin| {
            let package = loaded
                .repository
                .packages
                .iter()
                .find(|package| package.package.manifest.plugin_id().as_str() == *plugin)
                .unwrap_or_else(|| panic!("{plugin} is not in the repository"));
            let connector = package
                .package
                .connector
                .as_ref()
                .unwrap_or_else(|| panic!("{plugin} carries no connector table"));
            let corpus: serde_json::Value =
                kalareach_catalogue::read_json(&package.directory.join("fixtures/frames.json"))
                    .unwrap_or_else(|error| panic!("{plugin}: {error}"));
            let handshake: PinnedHandshake =
                serde_json::from_value(corpus["evidence"]["handshake"].clone())
                    .unwrap_or_else(|error| panic!("{plugin} pins no handshake: {error}"));
            (*plugin, connector, handshake)
        })
        .collect();
    let [(one, _, first), (other, _, second)] = &pinned[..] else {
        unreachable!("two distributions")
    };

    // Both builds call themselves the same agent, so a profile chosen by that name would be
    // chosen for either of them. The name is pinned as equal so nobody starts relying on it.
    assert_eq!(
        first.agent_info.name, second.agent_info.name,
        "{one} and {other} now report different agent names; the profiles can be told apart by name"
    );
    assert_ne!(
        first.agent_info.version, second.agent_info.version,
        "{one} and {other} pin the same version"
    );
    assert_ne!(
        first.session_capabilities, second.session_capabilities,
        "{one} and {other} advertise the same session operations"
    );

    for (plugin, connector, handshake) in &pinned {
        // The handshake a package pins is the one from the build its table was tested against.
        assert_eq!(
            handshake.agent_info.version,
            connector.protocol.tested_version.to_string(),
            "{plugin}: the pinned handshake came from another version"
        );
        // A table routes the session operations its own build advertises, and no other, except
        // to refuse one: a table that claimed an operation the build does not offer would be
        // describing the other distribution.
        for (capability, wire) in SESSION_OPERATIONS {
            let class = connector.route_for_wire_name(wire).map(|route| {
                serde_json::to_value(connector.classify(&route.method))
                    .ok()
                    .and_then(|value| value.as_str().map(str::to_owned))
            });
            if handshake.session_capabilities.contains(*capability) {
                assert!(
                    matches!(&class, Some(Some(class)) if class != "unsupported"),
                    "{plugin}: the build advertises {capability} and the table does not route {wire}"
                );
            } else {
                assert!(
                    class.is_none()
                        || matches!(&class, Some(Some(class)) if class == "unsupported"),
                    "{plugin}: the build does not advertise {capability} and the table routes {wire}"
                );
            }
        }
    }
}

/// Every agent section 12 bundles, and the plugins that carry its adapter, one for each profile.
///
/// This reads manifests. It says that no adapter was dropped, that no two adapters claim one
/// protocol, that every connector package in the repository is the profile of one of these agents,
/// and that nothing a control invokes reaches past what that package's own qualified table routes.
/// An agent whose vendor ships two profiles that cannot share a table has a package for each, and
/// neither is left out. That a host actually launches the terminal route, and that an agent
/// protocol is entered only when somebody selects it, are the host's and are not settled here.
const BUNDLED_AGENTS: &[(&str, &[&str])] = &[
    ("Claude Code", &["kalareach/claude-code"]),
    ("Codex", &["kalareach/codex"]),
    ("Gemini CLI", &["kalareach/gemini-cli"]),
    (
        "Kimi Code CLI",
        &["kalareach/kimi-code-cli", "kalareach/kimi-cli"],
    ),
    (
        "OpenCode",
        &["kalareach/opencode", "kalareach/opencode-attach"],
    ),
    ("Qoder CLI", &["kalareach/qoder-cli"]),
];

#[test]
fn every_bundled_adapter_is_present_and_declares_no_surface_of_its_own() {
    let loaded = packages::load(&root()).expect("the repository loads");

    // A connector package that is no bundled agent's profile would pass every check below by
    // never being read, so the list has to name all of them.
    let profiles: BTreeSet<&str> = BUNDLED_AGENTS
        .iter()
        .flat_map(|(_, plugins)| plugins.iter().copied())
        .collect();
    for package in &loaded.repository.packages {
        let plugin = package.package.manifest.plugin_id().to_string();
        assert!(
            package.package.connector.is_none() || profiles.contains(plugin.as_str()),
            "{plugin} carries a connector table and is the profile of no bundled agent"
        );
    }

    let mut protocols: BTreeMap<String, &str> = BTreeMap::new();
    for (agent_name, plugins) in BUNDLED_AGENTS {
        for agent in *plugins {
            // A missing upstream API is not a reason to drop an adapter, so every profile of
            // every bundled agent has a package in the repository whatever its protocol turned out
            // to support.
            let package = loaded
                .repository
                .packages
                .iter()
                .find(|package| package.package.manifest.plugin_id().as_str() == *agent)
                .unwrap_or_else(|| {
                    panic!("{agent} has no package, so an adapter for {agent_name} was dropped")
                });
            let manifest = &package.package.manifest;

            // Each adapter reads one vendor's own protocol. Two adapters sharing a pin would be
            // one control surface wearing two names, which is what a universal typed surface looks
            // like from here.
            if let Some(connector) = &package.package.connector {
                let name = connector.protocol.name.clone();
                if let Some(other) = protocols.insert(name.clone(), agent) {
                    panic!("{agent} and {other} both claim the protocol {name}");
                }
            }

            // Automatic native-composer insertion needs the bridge that makes it safe. Without
            // one, a package may declare a typed or a manual path, and not that one.
            if let Some(attachments) = &manifest.attachments.0 {
                assert!(
                    attachments.insertion != AttachmentInsertion::NativeComposer
                        || manifest.native_bridge.0.is_some(),
                    "{agent} claims native-composer insertion without a bridge"
                );
            }

            for action in &manifest.actions {
                // Nothing a control invokes writes into the terminal. The terminal stays the
                // person's, operated through the input grant they already hold.
                assert_ne!(
                    action.effect,
                    EffectClass::TerminalInput,
                    "{agent}: the action {} types into the native terminal",
                    action.id
                );
                match &action.implementation {
                    ActionImplementation::Presentation {}
                    | ActionImplementation::UpstreamCancel {} => {}
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
                    ActionImplementation::DecisionDestination { .. } => {
                        // An answer goes where this package's own table says an answer goes, and
                        // that is a method the table routes, for requests the table routes.
                        let destination =
                            package.package.connector.as_ref().and_then(|connector| {
                                connector
                                    .decision_destination
                                    .as_ref()
                                    .map(|destination| (connector, destination))
                            });
                        let Some((connector, destination)) = destination else {
                            panic!(
                                "{agent}: the action {} answers through a table that names no destination",
                                action.id
                            );
                        };
                        for method in [&destination.method, &destination.answers] {
                            assert!(
                                connector.routes.iter().any(|route| &route.method == method),
                                "{agent}: the action {} answers through {method}, which its own table does not route",
                                action.id
                            );
                        }
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
