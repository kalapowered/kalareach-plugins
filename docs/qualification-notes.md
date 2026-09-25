# Qualification notes

A connector table is a claim about how somebody else's protocol behaves. This file records what each
claim was checked against, so a reviewer can repeat the check instead of trusting the table.

One section per connector. Each pins the identities the package depends on, names the source those
identities came from and the date it was read, and says whether the claim was verified against a
live install, verified against a published schema or document, or left unverified with the reason.

The pinned identities are copied into the package's own immutable connector record, which is what a
host installs. This file is where the working came from.

Nothing here is recorded as verified unless it was actually checked. A claim nobody could check is
written as unverified and says why, because an unverified claim marked verified is worse than no
note at all.

Three kinds of evidence appear below, and they are not equally strong. A **document check** reads
what the vendor publishes. A **binary check** reads an identity or a name out of the installed
program. **Exercised behaviour** means the thing was run and watched. Nothing in this file is
exercised behaviour: no upstream was driven and no bridge was installed.

The evidence itself is pinned by content, so a later reader can tell whether they are looking at the
same bytes:

| Evidence | Platform | SHA-256 |
| --- | --- | --- |
| Codex 0.155.1, the native executable at `@openai/codex-darwin-arm64/vendor/aarch64-apple-darwin/bin/codex` | macOS, arm64 | `8eaf1ad12fe6bf89b1710330f58900014322c7c5af677e43be116d8ac5fc0a9e` |
| Codex 0.155.1, the launcher npm puts on the path at `@openai/codex/bin/codex.js` | any | `61b0194f3bb6534439c8d26a3ed57d0805f84b884588b761795323eeb92fcf70` |
| Codex generated schema, `codex_app_server_protocol.schemas.json` | | `f1f3591667d8dcf77352c04be5d0667153e492d1a378e4205d69e9af6f31c0c3` |
| Codex generated schema, `codex_app_server_protocol.v2.schemas.json` | | `f0402dc8ce8d278108f1e68e9d46ec7e59ddd9d153f5e70668d84d56f258dda3` |
| Claude Code 2.1.278 executable | macOS, arm64 | `bd245662fb8a0e321b3bf133e930371d6563c387527885f30b2613aef3ba14d6` |
| OpenCode 1.18.31, the native executable npm installs at `opencode-darwin-arm64/bin/opencode` | macOS, arm64 | `16c960ba77421da11b53e785f359b73f328a86118b48feb4af143db5d9afb198` |
| OpenCode 1.18.31, the OpenAPI document that build serves at `/doc` | | `46db986090aae41846cd6dbe16225a1d883f0bbcb4c48814008d3f6ce140aa5c` |
| Gemini CLI 0.60.0, the entry script npm puts on the path at `@google/gemini-cli/bundle/gemini.js`. The three rows below pin the inspected bundle chunks beside it | any | `fdff028b293149897b948a23b5d8da9e622127182a523be46d82cf267e7816f2` |
| Gemini CLI 0.60.0, `bundle/gemini-7INSUCPB.js`, one of the three chunks that carry every reverse agent-protocol method name the table routes | any | `161262ce223dc85a784bba05cd618e9ad3ea7065da557f417110dbba98b37052` |
| Gemini CLI 0.60.0, `bundle/gemini-LUNNHKPJ.js`, the second of those chunks | any | `98beff1e92ab73a131632832e5785ef761e2dca9257cb5a0bc8d606cfcd11ab2` |
| Gemini CLI 0.60.0, `bundle/gemini-ZTU7EMI3.js`, the third of those chunks | any | `b6498ba094610cdf303598bce0ed2c116aedcf0ab9d26d691be5dbb7cd8bf6ef` |
| Gemini CLI 0.60.0, `bundle/docs/hooks/reference.md`, the hook reference the vendor ships with that build | | `103bab9f0f8fd7251b97d06c6b7c4e52752427bf23cbacd1379f2aecaaf26e4c` |
| Gemini CLI 0.60.0, `bundle/docs/cli/acp-mode.md`, the agent-protocol document the vendor ships with that build | | `812af1001c110f474a624c3026c31e66519ca95c251d1f79679d8ebd56edd779` |
| Gemini CLI 0.60.0, `bundle/docs/extensions/reference.md`, the extension reference the vendor ships with that build | | `cf42a742ae7476faaf96a79fbcf462d86c1845fa6a2c0f39930d96559e284d34` |
| Gemini CLI 0.60.0, `bundle/chunk-M6NSK26M.js`, the core chunk the entry script loads: the hook registry, runner, aggregator and event handler | any | `5259439bfda18186b46bccf03916944c32c9dbfb532e01ff8b794e2e8e0afea3` |
| Gemini CLI 0.60.0, `bundle/chunk-GNCCSIBA.js`, the chunk that loads extensions and runs `/clear` | any | `cba32e6789d80b21e4dd96d8b147ceacdeee0fa99e3cb836cfc1e3a299531a56` |
| Gemini CLI 0.60.0, `bundle/chunk-GFHSA4BE.js`, the exit cleanup | any | `c140f38e2cff787fc7cb4282d31875d6e47d3ce6098a48e01b64d81aaf584d99` |
| Gemini CLI 0.60.0, `bundle/chunk-PJREBWY3.js`, the relaunch's spawn configuration | any | `12c9bcda4b9ddd42d92fbeb775d21ee63358c6f0d75f563eb083e7779989fe0f` |
| Gemini CLI 0.60.0, `bundle/chunk-OCLZG6C5.js`, the settings schema | any | `53bcd8f3b6fd478719bc31fec74a3ea61a8302b14ccf81f04a7be01a252331bd` |
| Gemini CLI 0.60.0, `bundle/interactiveCli-COQJBPC2.js`, the interactive session's start and end | any | `ba2e935e6bdb85f27816f883156309c012b838f56ff8ac5d959b9b6c7a08b6dd` |
| Kimi Code CLI 2.0.2 executable at `~/.kimi-code/bin/kimi` | macOS, arm64 | `c2204148f56c872539ac37bfe1868b597adee3606a3d7698ee9b9687aa537f11` |
| Qoder CLI 1.1.59 executable at `~/.qoder/bin/qodercli/qodercli-1.1.59` | macOS, arm64 | `c1b372d07083b98a2708ff23d4d462389fbf2bccad0d1317bb2970685f47c103` |
| Qoder CLI 1.1.63 executable, `qodercli` in the vendor's archive for that release, the same bytes as `~/.qoder/bin/qodercli/qodercli-1.1.63` | macOS, arm64 | `f3fd5ba1db73dd9591bc0bd0f7a3b1dd28fcc17ec4eb3eeb6ac2aa92cb91c0a0` |
| Qoder CLI 1.1.63, the vendor's archive `qodercli/releases/1.1.63/qodercli-darwin-arm64.tar.gz` | macOS, arm64 | `c9e6ca9248ec187b48df204bd68c987fbb59d0ea95e13ce44f380b4d8bd4a394` |
| Qoder CLI release manifest `qodercli/channels/manifest.json`, as fetched 2026-09-25, naming 1.1.63 | | `af4f0f5766a47b50c6aa2c4c410bcb3fee43bed296f38f403c73fcd8b4fb7bb0` |
| Qoder CLI installer at `https://qoder.com/install`, as fetched 2026-09-25, read and not run | | `1e850fa3375bdccd179215f91638f4065d912c5dbddbd346d0ba352bad8ca9be` |
| MoonshotAI kimi-cli 1.51.0, the wheel PyPI publishes as `kimi_cli-1.51.0-py3-none-any.whl`, installed for this qualification | any | `ffb9d0d4725844d36d6c78e8d3914a96b2d6a17678c217f3abe7336fb7e02d0e` |
| agent-client-protocol 0.8.0, `agent_client_protocol-0.8.0-py3-none-any.whl`, the protocol library kimi-cli 1.51.0 installs | any | `2d5712b88b3249dbd6148b24d32c6eb8992e5663f224db6291524ac80cca8037` |
| MoonshotAI kimi-cli 1.50.0, `kimi_cli-1.50.0-py3-none-any.whl`, read and not installed | any | `0341d283a3d5233128c1634c8201820a91cc09a1eaefc1d5075c481131b0b7e7` |
| MoonshotAI kimi-cli 1.52.0, `kimi_cli-1.52.0-py3-none-any.whl`, read and not installed | any | `7becbb9081e0c1194b481fe06087ceceaf12bad9f3798fcefc1e3dd2e80b90b2` |

The App Server is the native executable, which is what the schema above was generated from. The
launcher is listed beside it because it is what npm puts on the path.

Each package carries the same identities in its own `fixtures/frames.json`, which the manifest pins
by digest, so an installed copy holds the evidence its table was qualified against rather than a
reference to this file.

## kalareach/codex

### Identities

| What | Value | Source | Read | Status |
| --- | --- | --- | --- | --- |
| Executable | `codex` | `codex --version` on the qualification machine, reporting `codex-cli 0.155.1` | 2026-09-20 | Verified against a live install |
| Distribution | npm `@openai/codex` | `https://registry.npmjs.org/@openai/codex`, `dist-tags.latest` = `0.155.1` | 2026-09-20 | Verified against the registry |
| Protocol | Codex App Server, `codex-app-server` | `https://learn.chatgpt.com/docs/app-server` | 2026-09-20 | Verified against the published documentation |
| Protocol version tested | 0.155.1 | The JSON Schema written by `codex app-server generate-json-schema`, whose v2 document is titled `CodexAppServerProtocolV2`; both schema files are pinned by digest above | 2026-09-20 | Binary check |
| Qualified range | `=0.155.1` | This qualification | 2026-09-20 | The only release whose schema was read. The transport and the protocol are both documented as experimental, so the range admits nothing that was not tested |

### The declarative proxy contract

The App Server meets it, so the package ships no native bridge.

Over its default transport the App Server is JSON-RPC 2.0 over the process's standard streams, one
document per line. A request carries `method`, `params` and `id`; a response echoes the `id` with
either `result` or `error`; a notification omits the `id`. That gives the table a framing, a request
identifier at `id`, a method name at `method` and a response correlation by matching `id`, which is
every field the contract asks for. `RequestId` in the generated schema is a string or a 64-bit
integer, which is why the two are different identifiers and why an identifier keeps its JSON type.

Source: `https://learn.chatgpt.com/docs/app-server`, read 2026-09-20, and the generated schema above.
Status: verified against the published documentation and a live install.

### Method classification

Every wire name in `connector.json` but one was taken from the generated schema rather than from
prose: the client requests from the `ClientRequest` union, the notifications from
`ServerNotification`, and the reverse requests from `ServerRequest`.

Status: verified against a live install, with one exception. `process/spawn` appears in the
documentation as an experimental method behind the `experimentalApi` capability, and no request
union in either generated schema lists it. It is classified `unsupported` anyway, because refusing
it is what the table would do for a method it could not read. Status for that one entry:
documentation only, unverified against the schema.

The class beside each name is a judgement about what the method does, argued from the documented
behaviour and recorded as evidence in the table itself. Those judgements were not exercised against
a running App Server. Status: unverified, because qualifying a classification by running it means
starting turns and approving commands against a real account, which is not something a packaging
run should do.

### What is not qualified here

The native `--remote` terminal connection speaks WebSocket, over TCP or over a Unix socket with the
standard HTTP upgrade handshake, rather than the line-delimited JSON the table declares. The table
describes the leg between KalaReach and the App Server, and that leg alone is what meets the
declarative proxy contract. The leg between the native terminal and the gateway is the gateway's own
listener, and its framing is not expressible in a connector table today, because the SDK's `Framing`
vocabulary has no WebSocket member. Status: unqualified, and named here so nobody reads the table as
covering both legs or as establishing one App Server behind both clients.

`max_message_bytes` is 8 MiB. That is the bound this host enforces while reading frames, chosen
because a frame larger than it would not be read anyway. The vendor states no message limit, and a
thread history or an input collection large enough to pass it would fail the connection rather than
arrive in part. Status: a host bound, not a vendor fact.

`thread/resume` reads stored history. It does not establish that this process owns the live
execution, and nothing in the package treats it as though it did. Status: verified against the
published documentation.

Volatile forwarding is declared as untested, which is what `volatile_forwarding: false` means. Until
it is tested, the unchanged terminal integration is the supported path when receipt storage fails.

The six behaviours the Codex row names, which are competing approvals, thread subscriptions,
`turn/steer`'s `expectedTurnId`, `serverRequest/resolved`, reconnect and an uncertain `turn/start`
result, are protocol behaviours, and they are covered here only as far as a package can cover them.

`fixtures/frames.json` pins fifteen frames across those situations, written from the documentation's
examples and completed against the generated schema's required members, and the test
`every_pinned_frame_is_read_the_way_the_table_says` under `cargo test` reads every one of them through
this package's own table: the method it finds, the route and class it reaches, the identifier it
extracts with its JSON type, the members the situation is about, and that the correlation path finds
the same identifier the request path does. That is a check of the table, and of nothing else. Status:
asserted by `every_pinned_frame_is_read_the_way_the_table_says` in `pipeline/tests/packages.rs`, which
runs under `cargo test`. It checks the table's reading of pinned frames and nothing about a host.

What those frames do not establish is state. Two approval frames are two frames, not two requests
pending at once; an initialize and an interrupt are two frames, not a reconciled reconnect; a
`turn/start` with no response beside it is a frame, not an uncertain result that was handled. The
frame corpora stay frames; stateful sequence fixtures need pipeline/src/fixtures.rs to carry a
sequence rather than a frame, and it does not. Recorded, not built. Driving a connection
through those transitions and checking what the host does belongs to the gateway.
Status: not covered here, and the requirement row stays open for that half.

Oversized frames and replay are in the same position. The table declares the bound; what the host
does when a frame passes it, and that a connection is never reconnected and replayed around an
unknown request, are the gateway's to establish. Status: not covered here.

## kalareach/claude-code

### Identities

| What | Value | Source | Read | Status |
| --- | --- | --- | --- | --- |
| Executable | `claude` | `claude --version` on the qualification machine, reporting `2.1.278 (Claude Code)` | 2026-09-20 | Verified against a live install |
| Distribution | npm `@anthropic-ai/claude-code` | `https://registry.npmjs.org/@anthropic-ai/claude-code`, `dist-tags.latest` = `2.1.278` | 2026-09-20 | Verified against the registry |
| Protocol | Claude Code Channels, `claude-code-channels` | `https://code.claude.com/docs/en/channels-reference` and `https://code.claude.com/docs/en/channels` | 2026-09-20 | Verified against the published documentation |
| Protocol version tested | 2.1.278 | The three channel notification names are present in the installed 2.1.278 executable, pinned by digest above | 2026-09-20 | Binary check |
| Qualified range | `=2.1.278` | This qualification | 2026-09-20 | The only release checked. The documented behaviour floor is 2.1.234, the release that sends permission requests only to servers it registered as channels, treats an undeclared permission capability as undeclared, and masks credentials in the relayed fields; that floor is what the bridge recipe's `application_range` is written for, and it is not a claim that any release between it and 2.1.278 was tested |

### The declarative proxy contract

Channels cannot meet it as a proxied protocol, which is why this package ships a bridge.

A channel is an MCP server that Claude Code spawns as a subprocess of its own and talks to over that
subprocess's standard streams. There is no documented way to put the gateway between the two, and
nothing in the protocol identifies a proxy. The only place KalaReach can stand is where the channel
server stands, so the bridge registers the core forwarder as that server.

Source: `https://code.claude.com/docs/en/channels-reference`, read 2026-09-20.
Status: verified against the published documentation.

What the table then describes is the private exchange between the forwarder and the worker, not the
MCP handshake. The forwarder terminates MCP; only the channel frames cross to the broker. Status:
core's forwarder does this at core `e2f867b3` (`crates/kr-hook/src/claude_code/channel.rs`), and
core's tests check it against a stand-in application; it was not observed against a live Claude
Code.

The table routes, classifies and says how a relayed approval is answered. Its decision destination
names `channel.permission-request` as the request it answers, so a message of that method is a
pending approval and no other message is. It names `channel.permission` as the answer, which repeats
the request's own `params.request_id` and carries a `params.behavior` of `allow` or `deny`, the only
two decisions it maps. That declaration is the whole interpretation: this package ships no
component, so nothing rewrites the relayed `tool_name`, `description` and `input_preview`. A
package's own document cannot carry an `approval_ref`, since that node names a runtime resource.
Status: the notification shapes are a document check. A test in `pipeline/tests/packages.rs` writes
the answer for the pinned request from the table and gets the pinned answer in
`fixtures/frames.json`, with that request's identifier, and no other pinned message gets an answer.

### Method classification

Three methods, all of them JSON-RPC notifications:

- `notifications/claude/channel` delivers a message into the session.
- `notifications/claude/channel/permission_request` relays a pending tool approval out, with
  `request_id`, `tool_name`, `description` and `input_preview`.
- `notifications/claude/channel/permission` answers one, with `request_id` and `behavior`.

All three names are present in the installed 2.1.278 executable, as is the `request_id` field and
the five-letter identifier alphabet the documentation describes. Status: a binary check for the
names, and a document check for the field shapes.

Because all three are notifications, none of them carries a JSON-RPC `id`. The correlation
identifier is `params.request_id`, which is where the table puts it. Status: verified against the
published documentation.

The package registers one answering action, `approval.answer`, with the effect `approval.respond`,
and it answers through the table's decision destination. A call names the pending request it
answers, and the package contract refuses a call that names none. The identifier in the answer is
the one that request carried, never a value a caller supplies, and the value comes from the table's
mapping. The vendor's own documented failure mode is the one this is meant to close: a reply in the
wrong format falls through to Claude as an ordinary message, and a reply naming an identifier nobody
issued is dropped in silence. Inside KalaReach neither becomes a message, because the message path
is a separate action with its own effect class, and an answer carries only a mapped decision for the
request it names. Status: the notification shapes are a document check, and `fixtures/frames.json`
checks that the table reads them the way the record says. `pipeline/tests/packages.rs` checks, under
the package contract at core `e2f867b3`, that a call naming no request is refused and that every
decision the action offers has a mapped value.

The package requests `approval.decode` and `approval.respond`, because interpreting a native request
and answering one are separate grants. Its document draws Allow and Deny for an actor who holds
`agent.approval.respond` while an approval is pending. The document is written before any request
exists, so the controls test that some approval is pending rather than naming one. The request a
press answers is the one the call names: a client that draws the controls beside a request binds the
press to that request, and the host checks the named request again when the call arrives. The
controls are enabled only while the host reports `approval.respond` as qualified and available on
the binding, and never during volatile-native operation. Status: the declarations validate under the
package contract at core `e2f867b3`, and `fixtures/conformance.json` states who sees the controls
and who can use them. No live Claude Code was answered through them, and no running host carried an
answer: at that core revision the host checks the named request and then refuses to transmit a
plugin action's effect, so the request stays pending and is answered in the terminal, where Claude
Code also asks (`docs/plugins/sdk.md` at core `e2f867b3`).

Approval authority comes only from the native request. A pending approval is a relayed
`channel.permission-request` and nothing else, so text that reads like a permission prompt, on the
terminal or in a `Notification` hook's report, makes none, and the controls stay hidden while a
session waits on such a prompt. Status: a fixture check. In `fixtures/conformance.json`, a session
that waits for a person with no pending approval shows no approval control.

### Runtime installation gates

Registering the channel does not enable it. Every one of these is decided where Claude Code runs:

1. The session names the server or its plugin at launch, with `--channels` or with
   `--dangerously-load-development-channels`. Being in the MCP configuration is not enough.
2. The plugin is on the effective allowlist, which is Anthropic's list unless an organisation
   replaces it with `allowedChannelPlugins`. The development flag bypasses the allowlist alone.
3. The organisation's `channelsEnabled` setting permits channels at all.
4. The session authenticates through claude.ai or a Console API key. Channels are documented as
   unavailable on Amazon Bedrock, Google Cloud's Agent Platform and Microsoft Foundry.
5. The server declares `claude/channel`, and `claude/channel/permission` as well before any approval
   is relayed to it.
6. The negotiated MCP protocol revision is one this version registers a channel over.
   `https://code.claude.com/docs/en/channels` names revision 2026-07-28 as one that stops a channel
   registering under the v2 MCP client runtime when `MCP_PROTOCOL_NEGOTIATION` is `auto`.
7. The feature reaches this installation at all. The same page describes channels as a research
   preview whose availability is rolling out gradually, and the installed executable refuses a
   registration when the feature is not available to the session, independently of the settings
   above.

8. The skills directory is scanned at all. The installed executable carries managed marketplace
   policy settings, `strictKnownMarketplaces` and `blockedMarketplaces`, that can suppress that
   scan and leave the registration on disk and unloaded.

Sources: `https://code.claude.com/docs/en/channels` and
`https://code.claude.com/docs/en/channels-reference`, read 2026-09-20, for conditions 1 to 6;
conditions 7 and 8 are binary checks of the installed executable. Status: document checks and two
binary checks; none of them exercised. Whether a channel actually registered on a given machine is
something only that machine can report. Whether any of them holds on a given machine is not something this package
can claim.

Claude Code applies whichever answer arrives first and drops the other. It sends the channel no
notification of that outcome: the reference describes the local dialog closing and the pending
remote request being dropped, and an identifier nobody issued being dropped in silence. So a
delivered answer proves the answer was sent, not that it was the one applied. Status: verified
against the published documentation.

### The bridge, and what is unverified about it

The recipe installs three files into Claude Code's own directory and adds one settings key:

Every destination is relative to the user's own Claude Code directory, which is where
`settings.json` lives and under which `skills/` sits.

| Installed | Documented location | Source | Read | Status |
| --- | --- | --- | --- | --- |
| Plugin manifest | `.claude-plugin/plugin.json` under a skills directory, loaded in place with no marketplace | `https://code.claude.com/docs/en/plugins-reference` | 2026-09-20 | Document check |
| Channel server | `.mcp.json` in the plugin root | `https://code.claude.com/docs/en/plugins-reference` | 2026-09-20 | Document check |
| Hooks | `hooks/hooks.json` in the plugin root | `https://code.claude.com/docs/en/plugins-reference` | 2026-09-20 | Document check |
| `enabledPlugins."kalareach-channels@skills-dir"` | `settings.json` under the user's Claude Code directory | `https://code.claude.com/docs/en/plugins-reference` and `https://code.claude.com/docs/en/hooks` | 2026-09-20 | Document check |

A plugin in a skills directory loads as soon as it is there, and its default enablement is on unless
the manifest says otherwise. The installed manifest sets `defaultEnabled` to `false`, so the
settings key is what turns the registration on and removing that key is what turns it off. An
explicit setting at another scope overrides the default, and a session that is already running keeps
what it loaded until its plugins are reloaded or it starts again. Source:
`https://code.claude.com/docs/en/plugins-reference`, read 2026-09-20. Status: document check.

Each of the three files uses its own documented location rather than the inline form the manifest
also accepts, because the documentation does not fix the inline shape and a registration that loads
is worth more than a shorter one.

The five hook events are `SessionStart`, `SessionEnd`, `PostToolUse`, `PostToolUseFailure` and
`Notification`. The hooks reference lists, per event, what an exit code of 2 does, and for these five
it refuses nothing: the action has already happened or the code is ignored. That is why they were
chosen, and it is the whole of what the event list guarantees. It is not a guarantee that a hook
cannot interfere: the same reference documents a common `continue` field, and a handler that answers
`{"continue": false}` stops the session on the events that read a hook's JSON output, which is most
of them; `SessionEnd` is one that discards it. A hook here observes because the
forwarder answers `{}`, an object that sets nothing, and exits 0, not because the event cannot carry
an answer. Each registration also
carries a `timeout`, one second for `SessionEnd` and five for the rest, against a documented command
default of 600 seconds. Source: `https://code.claude.com/docs/en/hooks`, read 2026-09-20. Status:
document check.

What core provides, and what is unverified about it:

- Core's native bridge executor, `crates/kr-controller/src/catalogue/native_bridge/` at core
  `bf075f9eb`, installed this release's recipe into an isolated Claude Code directory on the
  qualification machine, and the qualified executable read the registration. Nothing of the person's
  was used and no prompt was sent: Claude Code 2.1.278, the executable pinned above, ran in a
  terminal with `CLAUDE_CONFIG_DIR` set to that directory, a dummy API key and a refused base URL,
  under a sandbox that denied the keychain, the person's own Claude Code paths, the external volume
  and outbound TCP, and was stopped after 14 seconds. A recorder stood first on its `PATH` as
  `kr-hook`. Before the installation, Claude Code started no forwarder. Installed, it ran
  `kr-hook claude-code hook` for `SessionStart`, started `kr-hook claude-code channel` as the
  plugin's MCP server, and ran `kr-hook claude-code hook` for `SessionEnd` when it was stopped.
  After the executor's removal it started none. The installation added the three files, the
  directories that hold them and the settings key, and nothing else; the removal took out exactly
  those and left `settings.json` byte for byte as it was, the other settings included. Claude Code
  wrote its own records of the plugin beside them, `plugins/installed_plugins.json` and
  `plugins/data/kalareach-channels-skills-dir`, which the recipe does not name and its removal does
  not touch. Status: verified by two runs without an account, 2026-09-25.
- The executor reads each `claude` executable on the daemon's search path without running it, and
  refuses the recipe unless a signed qualification record names that executable's digest at a
  version inside the recipe's range. No release carries such a record yet, so core refuses the
  recipe and writes nothing. The runs above supplied the record this qualification publishes: the
  digest pinned above, at 2.1.278. Status: verified by core's tests and by the runs above.
- Not verified: a session with an account; the channel registering under
  `--dangerously-load-development-channels`; whether a session that is already running picks the
  registration up without a restart; and what Claude Code does with its own plugin records after the
  removal. A removal keeps and reports a file changed since it was installed; that is verified by
  core's tests against a stand-in directory, not against a live install.
- The host contract is core's `kr-hook` forwarder and the worker's gateway, at core `e2f867b3`
  (`crates/kr-hook` and `docs/bridges/claude-code/README.md`):
  - The recipe registers `kr-hook claude-code channel` and `kr-hook claude-code hook`, and the
    forwarder accepts both. A command line it does not accept, such as an unknown application or
    surface, an extra argument or an undeclared flag, is a usage error: it is refused on standard
    error with exit code 64, before anything is read or connected, because Claude Code reads a
    hook's exit code 2 as a request to block (`crates/kr-hook/src/cli.rs`).
  - The launch's environment names one file, the registration, in `KR_REGISTRATION`, a path. The
    registration names the owner-only credential file beside it, and the forwarder reads the
    credential only from a file in the registration's own directory that no other user can read, so
    no credential travels in the environment. That holds from core `571f197a`, where the worker's
    launch sets `KR_REGISTRATION` alone (`crates/kr-hook/src/registration.rs`,
    `crates/kr-worker/src/broker/attach.rs`); at `e2f867b3` the environment also named the
    credential file, in `KR_CREDENTIAL`.
  - The worker admits a forwarder only when the kernel names the connecting process, its parent
    chain leads to the Claude Code process the worker launched with every link checked by start
    identity, it presents the launch credential, and the installation record names its application
    (`claude-code`), the surface it declares (`hook` or `channel`) and the forwarder executable it
    runs (`docs/bridges/claude-code/README.md`, "Admission").
  - Every hook writes exactly `{}` to standard output and exits 0 within 500 milliseconds, and never
    waits for a person (`crates/kr-hook/src/claude_code/hook.rs`). The hook registrations stay in
    exec form, a `command` with an `args` list and no shell between Claude Code and the forwarder,
    because only a hook Claude Code started itself selects the worker's thread
    (`docs/bridges/claude-code/README.md`, "Hooks"). `crates/kr-hook/tests/fixtures.rs` checks that
    form and pins the recipe's three files by their digests.
  - The channel declares `claude/channel` and `claude/channel/permission` only once the worker has
    admitted it, and nothing outside a launch. It negotiates MCP revision 2025-11-25 at the newest.
    It relays a `permission_request` with its four fields, `request_id`, `tool_name`, `description`
    and `input_preview`, and nothing else, and it forwards to Claude Code only the `channel` and
    `permission` frames it has checked: a verdict only for a request it relayed and has not answered
    (`crates/kr-hook/src/claude_code/channel.rs`).

  Status: verified by core's tests against a stand-in application, not against a live Claude Code.
- Two bounds hold at that core revision. The host admits no bridge on Windows: it publishes the
  launch credential as a file only on Unix (`crates/kr-worker/src/broker/process.rs:277`), and a
  launch that cannot publish it starts nothing (`crates/kr-worker/src/broker/attach.rs:774`). Only
  core's tests call the gateway's `accept_bridge` and `observe_hook`
  (`crates/kr-worker/src/broker/attach.rs:981` and `:1048`), so no running host serves this bridge at
  that revision.
- Claude Code documents `CLAUDE_CODE_SUBPROCESS_ENV_SCRUB=1` as a setting that strips every variable
  it recognises as a credential from the hooks and MCP stdio servers it starts. Source:
  `https://code.claude.com/docs/en/env-vars`, read 2026-09-25. Status: document check. From core
  `571f197a` the forwarder needs `KR_REGISTRATION` alone, which names no credential, and core's
  bridge page says the setting keeps it (`docs/bridges/claude-code/README.md`, "Finding the
  worker"). Status: document check of core's page; which names the setting strips was not checked
  here. Where it did not keep `KR_REGISTRATION`, the hooks would still answer `{}` and report
  nothing, and the channel would complete its handshake and declare nothing, as outside a launch.
- The forwarder finds its launch only through the environment Claude Code gives the processes it
  starts. Core's bridge page cites the hooks reference for a hook inheriting Claude Code's
  environment, and relies on Claude Code passing the same variables to the MCP servers it starts.
  Status: unverified against a live install.
- None of the three installed files carries a session identifier or a secret, which can be checked by
  reading them, and that much is verified. Binding the registration to the launch KalaReach made and
  to the worker's private exchange is the admission above, and it was not observed against a live
  install.

### Pipeline changes this package required

`pipeline/tests/release.rs`: target paths are derived rather than written out, because this
package's version moved to 0.2.0.

`pipeline/tests/packages.rs`: the declarative action forms this repository has reviewed include the
decision destination, which this package is the first to use. An answer goes through a destination
the package's own table declares, for a method and a request the table routes.

## kalareach/opencode

### Identities

| What | Value | Source | Read | Status |
| --- | --- | --- | --- | --- |
| Executable | `opencode` | `opencode --version` on the qualification machine, reporting `1.18.31` | 2026-09-20 | Verified against a live install |
| Distribution | npm `opencode-ai` | `https://registry.npmjs.org/opencode-ai`, the 1.18.31 release, installed into a directory owned by this qualification and nowhere else | 2026-09-20 | Verified against the registry |
| Protocol | The shared-server API family's durable event stream, `opencode-server-v2-events` | The OpenAPI document that build serves at `GET /doc`, pinned by digest above | 2026-09-20 | Binary check |
| Protocol version tested | 1.18.31 | The same document | 2026-09-20 | Binary check |
| Qualified range | `=1.18.31` | This qualification | 2026-09-20 | The only release whose schema was read. Both API families are served from one build and the earlier one is still there, so the range admits nothing that was not read |

### The declarative proxy contract

The event stream meets it, so the package ships no native bridge for that leg.

`GET /api/event` is a server-sent event stream whose every message is one JSON document with `id`,
`type` and `data`, and whose `id` matches `^evt_`. That gives the table a framing and a method name
at `type`. The identifier at `id` is the durable event identifier the stream orders and resumes by,
beside `durable.aggregateID` and `durable.seq`; nothing travels host to upstream on this leg, so
there is no request here for a response to answer, and the correlation the manifest requires repeats
the same path. That is the whole of what correlation means here, and it is not a claim that a
request was matched to a reply.

The request leg does not meet it. Asking that server to do something is an HTTP request, and its
method is a verb and a path (`POST /api/session/{sessionID}/interrupt`, `v2.session.interrupt`)
rather than a member of a document. A connector table names a method by a bounded path into a
decoded message, and `Framing` has no HTTP member, so the request leg is not expressible in a
connector table today. Status: named here as outside the table, so nobody reads the table as
covering both legs. Interrupting a turn therefore goes through the broker's own cancellation against
the bound execution rather than through a route in this table.

### The two API families

One build answers both, and the qualification turns on telling them apart.

| | This package's profile | The other profile |
| --- | --- | --- |
| Event stream | `GET /api/event`, schema `V2Event` | `GET /event`, schema `Event` |
| Envelope payload | `data` | `properties` |
| Operation identifiers | prefixed `v2.` | unprefixed |
| Answering a permission | `POST /api/session/{sessionID}/permission/{requestID}/reply` | `POST /session/{sessionID}/permissions/{permissionID}` |

The server-wide routes under `/global/` are unprefixed, so they belong to neither family
exclusively, and the table makes no claim about them.

The `V2Event` union carries 88 names. The `Event` union carries the same 88 and one more,
`server.instance.disposed`; `global.disposed` is in both. So a name is not a discriminator, and the
payload member is the only thing that differs: this family puts it under `data` and the earlier one
under `properties`. A table has no payload-shape field, so it cannot refuse a shared name by reading
it. What it can do is name the one event the earlier family alone publishes and declare it
`unsupported`, which is what this table does, so that seeing it is an answer rather than a default.
`fixtures/frames.json` pins one envelope of each kind, and
`a_frame_from_the_other_api_family_is_unsupported_or_carries_no_payload` in
`pipeline/tests/packages.rs` asserts the class the table gives each of them and that each frame's
own expectation reads only the earlier family's payload member.

Which family a connection belongs to is therefore settled before it opens, from the installed
version and the served schema rather than from a name on disk. This package's record carries both
pins, and a host whose evidence does not match them reports the capability as `incompatible`. The
case in `fixtures/conformance.json` states what this package draws then: nothing, so the terminal
path is what is left, and `kalareach-catalogue validate` evaluates that case against this package's
own predicates on every run. Reading the installed evidence and comparing it against those pins is
the host's half of that contract. This package neither performs it nor tests it, and nothing here
records it as done.

Source: the OpenAPI document above, read 2026-09-20. Status: binary check.

The earlier family is a separate profile, not an unsupported one. Its package is
`kalareach/opencode-attach`, with its own immutable record, match rules and version line, qualified
against the same document's unprefixed half.

### Method classification

Eighty-nine event names: every one in that document's `V2Event` union, with nothing added and
nothing left out, plus the one name the `Event` union carries and `V2Event` does not. Eighty-four
are observations: an event on this stream reports what the session already did.

Five are not observations, and four of them are the ones a gateway has to be careful with.
`tui.session.select` moves the attached terminal to another conversation, which changes the
execution owner a binding observes; `tui.toast.show` writes on that terminal's screen. Both are
mutations. `tui.prompt.append` writes into the native composer, and `tui.command.execute` carries a
command vocabulary that includes `prompt.submit` and `session.interrupt`. Both are declared
unsupported, because forwarding either would be automatic composer insertion or a typed control
surface over somebody's terminal, and this package qualified neither. `server.instance.disposed` is
unsupported for the different reason above.

No event on this stream carries a credential, so no entry is classified `credential`. Provider
authentication happens on HTTP routes, which this table does not cover.

Status: the names and their payload shapes are a binary check of the served document. The class
beside each name is a judgement argued from the documented payload and recorded as evidence in the
table itself; no classification was exercised against a running turn, because that needs a vendor
account.

### What is not qualified here

`max_message_bytes` is 8 MiB, the bound this host enforces while reading frames. The vendor states
no limit. Status: a host bound, not a vendor fact.

Volatile forwarding is declared untested, which is what `volatile_forwarding: false` means. Until it
is tested, the unchanged terminal integration is the supported path when receipt storage fails.

The agent-protocol subcommand this build also offers is a different surface from the server API and
is not what this package reads. Status: out of scope here.

Nothing was driven: no turn was started, no permission was answered, and no account was signed in
to. The server was started once, on loopback, with a home directory belonging to this qualification,
only to read the document it serves.

## kalareach/gemini-cli

### Identities

| What | Value | Source | Read | Status |
| --- | --- | --- | --- | --- |
| Executable | `gemini` | `gemini --version` on the qualification machine, reporting `0.60.0` | 2026-09-20 | Verified against a live install |
| Distribution | npm `@google/gemini-cli` | `https://registry.npmjs.org/@google/gemini-cli`, the 0.60.0 release, installed into a directory owned by this qualification and nowhere else | 2026-09-20 | Verified against the registry |
| Protocol | The agent protocol over standard streams, `gemini-cli-acp` | The handshake that build answers | 2026-09-20 | Binary check |
| Protocol version tested | 0.60.0 | `agentInfo` in that handshake: `{"name":"gemini-cli","title":"Gemini CLI","version":"0.60.0"}`, and `protocolVersion` 1 | 2026-09-20 | Binary check |
| Qualified range | `=0.60.0` | This qualification | 2026-09-20 | The only release probed |
| Bridge range | `=0.60.0` | This qualification: every hook fact below was read from that release | 2026-09-25 | The only release checked |

### The flag

The published documentation disagrees on `--acp` and `--experimental-acp`. The pinned binary settles
it: its own help lists both, describes `--acp` as "Starts the agent in ACP mode" and
`--experimental-acp` as "Starts the agent in ACP mode (deprecated, use --acp instead)". The
qualification used `--acp`, and that build answered.

Source: `gemini --help` on 0.60.0. Status: binary check, performed.

### The declarative proxy contract

The agent protocol meets it, so no bridge carries it; the bridge below is for hooks.

It is JSON-RPC 2.0 over the started process's standard streams, one document per line. A request
carries `method`, `params` and `id`; a response echoes the `id`; a notification omits it. That gives
the table a framing, an identifier at `id`, a method at `method` and a correlation by matching `id`.

Status: verified by sending requests to that build and reading what it wrote back.

### Method classification

Seventeen methods. Each host-to-upstream name was sent to the pinned build over its standard
streams, and each upstream-to-host name is present in that build's own files.

| Sent | What 0.60.0 answered |
| --- | --- |
| `initialize` | A result: protocol version 1, four authentication methods, `agentInfo`, and prompt capabilities for image, audio and embedded context |
| `authenticate`, `session/load`, `session/prompt`, `session/set_mode`, `session/set_model` | An error about the request's own parameters, which is the method answering |
| `session/new` | A result with a session identifier, the four approval modes and the model list |
| `session/list`, `session/resume`, `session/close`, `session/delete`, `session/fork` | Method not found. This build supports none of them, and the table does not route them |
| `session/cancel` | Method not found as a request. The specification defines it as a notification, and every build probed here answers the same way; it was not exercised as a notification |
| A name nobody implements | Method not found, which is what an unrouted name gets |

The nine upstream-to-host names (`session/update`, `session/request_permission`, the two `fs/`
methods and the five `terminal/` methods) are present in that build's own bundle. They are the
requests the agent makes of its client, so the agent does not answer them itself and probing them
from the client side reports method not found, which is the correct answer rather than evidence of
absence.

Status: a binary check for every name. The class beside each name is a judgement argued from what
the method does, recorded as evidence in the table itself, and not exercised against a live turn.

### The hooks bridge

The package installs an extension, `kalareach`, and nothing else: `bridge/gemini-extension.json` at
`extensions/kalareach/gemini-extension.json`, `bridge/hooks.json` at
`extensions/kalareach/hooks/hooks.json` and `bridge/gemini-extension-install.json`, the extension's
install record, at `extensions/kalareach/.gemini-extension-install.json`, all under the user's own
Gemini CLI directory. The record names `/dev/null/kalareach` as a `local` source; it is installed
first and removed last. The hooks file registers `kr-hook gemini-cli hook` for `SessionStart` and
`Notification` with a timeout of 5000 milliseconds and for `SessionEnd` with 1000. Everything below
was read from 0.60.0 on 2026-09-25. A binary check here is either the installed bundle read (the
chunks pinned by digest above) or the build started with a home directory and a working directory
belonging to this qualification, the network denied, no account and no model turn, and what its
hooks recorded read back. The runs are in the qualification's own log, which also holds every probe
script. The build's hook events are `BeforeTool`, `AfterTool`, `BeforeAgent`, `AfterAgent`,
`Notification`, `SessionStart`, `SessionEnd`, `PreCompress`, `BeforeModel`, `AfterModel` and
`BeforeToolSelection`, from the hook reference it ships and the names in its chunks.

| Fact | Source | Status |
| --- | --- | --- |
| Gemini CLI loads every directory under `extensions/` in its own directory (`~/.gemini`, or `$GEMINI_CLI_HOME/.gemini`) that holds a `gemini-extension.json` with a `name` and a `version`, and enables it unless `extensions/extension-enablement.json` or the `--extensions` flag says otherwise | The extension manager in `chunk-GNCCSIBA.js`; runs with the package's own files in place | Binary check, read and run |
| An extension's install record is `.gemini-extension-install.json` in its directory. Where `security.allowedExtensions` lists any pattern, every directory under `extensions/` is checked for a record before its manifest: one with no record, or a record naming no `source`, is an error that stops Gemini CLI before any session starts (exit 1, "The .gemini-extension-install.json file is missing or misconfigured"), whether it holds an extension, another file or nothing. A directory with a record loads only when a pattern matches its source, and is otherwise skipped with a warning while the session runs; one whose record is left without a manifest is skipped with a warning too. Without the setting, the record plays no part in loading | `_buildExtension` and `loadInstallMetadata` in `chunk-GNCCSIBA.js`; runs with probe files: no record and a list (exit 1), a record and a list that does not match it (skipped, the session ran), a record and no list, and a record and a list that matches it (the hook ran); the package's three files with a matching list, with one that does not match and with none; and what a removal could leave, under a list: an empty directory and one holding only a file of the person's (both exit 1), and the record with a file of the person's, under a matching list and under another (both skipped with a warning, the session ran) | Binary check, read and run |
| The patterns are tested against the record's source as a real path, or, where that path does not exist as written, as resolved from the session's working directory; `~` is not expanded, so `~/.gemini/extensions/kalareach` would be tested as `<working directory>/~/.gemini/extensions/kalareach`. The package's `/dev/null/kalareach` is absolute and nothing can exist under it, so it is tested as written wherever the session runs, and `^/dev/null/kalareach$` matches it | `getRealPath` in `chunk-M6NSK26M.js`, which the extension manager imports; the matching runs above, with both sources | Binary check, read and run |
| A record's source, not the extension's name, is what identifies an extension that has one, which scopes that extension's own settings; this extension declares none. For a `local` record, the interactive CLI's update check reads the manifest at the source and offers an update when its version differs; a source it cannot read reports "not updatable", the status an extension with no record gets, and nothing is shown. A relative source is read from the session's working directory, so a project could supply that manifest: with `~/.gemini/extensions/kalareach` as the source and a manifest of another version planted at `<project>/~/.gemini/extensions/kalareach/`, the CLI said "You have 1 extension with an update available. Run "/extensions update kalareach"."; with `/dev/null/kalareach` and the same project, it said nothing. The record sets no `autoUpdate`, so no update is applied without the person asking | `getExtensionId`, `checkForExtensionUpdate`, `checkForAllExtensionUpdates` and `useExtensionUpdates` in `chunk-GNCCSIBA.js`; the two planted runs; interactive runs with the package's three files, which showed nothing about the extension | Binary check, read and run |
| An extension's `hooks/hooks.json` is loaded while `hooksConfig.enabled` holds, which it does by default, and its hooks run beside the hooks in the settings files, never in place of them | `loadExtensionHooks` in `chunk-GNCCSIBA.js`, the hook registry in `chunk-M6NSK26M.js`, the settings schema in `chunk-OCLZG6C5.js`; a run in which the extension's `SessionStart` hook and one in the user's `settings.json` both ran | Binary check, read and run |
| Every hook in a settings file, the user's own included, counts as a project hook and does not run in a folder the person has not trusted; an extension's hook runs there | The hook registry and runner in `chunk-M6NSK26M.js`; an interactive run answered "Don't trust", in which the extension's `SessionStart` hook ran and the user's did not | Binary check, read and run |
| Headless mode refuses a folder the person has not trusted and exits 55 | A run in a fresh folder | Binary check, run |
| A hooks key in the user's `settings.json` holds the person's own hooks for that event, so adding KalaReach's there replaces theirs | The hook reference the build ships, `bundle/docs/hooks/reference.md` | Document check |
| A hook's command runs as `bash -c "<command>"`, bash found on the path, with no shell of Gemini CLI's own | `getShellConfiguration` and the command runner in `chunk-M6NSK26M.js` | Binary check, read |
| Bash runs a command of plain words in its own process, so the hook's parent is Gemini CLI's node process | Runs through Homebrew's bash 5.3 and through `/bin/bash` 3.2, each recording its parent from the kernel | Binary check, run |
| The hook's environment is Gemini CLI's own, with `GEMINI_SESSION_ID`, `GEMINI_PROJECT_DIR`, `GEMINI_CWD` and `CLAUDE_PROJECT_DIR` added: `KR_REGISTRATION` and `KR_SESSION` arrived | A run; `sanitizeEnvironment` in `chunk-M6NSK26M.js` | Binary check, run |
| Redaction is off unless `security.environmentVariableRedaction.enabled` is set; then no redacted name pattern matches `KR_REGISTRATION`. Where `GITHUB_SHA` is set or `SURFACE` is `Github`, every variable not on an allow list is removed, `KR_REGISTRATION` with them, and the forwarder then answers `{}` and reports nothing | `sanitizeEnvironment` and `shouldRedactEnvironmentVariable` in `chunk-M6NSK26M.js` | Binary check, read |
| `SessionStart` carries `source` (`startup`, `resume` or `clear`), `SessionEnd` carries `reason` (`exit`, `clear`, `logout`, `prompt_input_exit` or `other`), and each carries `session_id` | The hook reference; the payloads of a run's `SessionStart` and `SessionEnd` | Binary check, run, and document check |
| `Notification` fires with `notification_type` `ToolPermission`, a `message` and `details` when a tool asks for permission, and the prompt waits for the hooks to finish | The hook reference; `resolveConfirmation` and `fireToolNotificationEvent` in `chunk-M6NSK26M.js` | Document check and binary check, read; not observed, because it needs a model turn |
| On `SessionStart`, `SessionEnd` and `Notification` the build ignores `decision` and `continue`; it reads `systemMessage` on all three and `hookSpecificOutput.additionalContext` on `SessionStart` | The hook reference; the event handler and `HookSystem` in `chunk-M6NSK26M.js` | Document check and binary check, read |
| When a hook's standard output is empty its standard error is parsed as its answer, and plain text with an exit code other than 0 and 1 becomes `{"decision":"deny"}`. A failed hook's answer is still merged, and on `AfterTool` a refusal replaces the tool's result with "Tool result blocked" and its reason | `executeCommandHook` and `convertPlainTextToHookOutput`, `HookAggregator`, and `executeToolWithHooks` in `chunk-M6NSK26M.js` | Binary check, read |
| `{}` with exit 0 is parsed as an object that sets nothing, is merged with any other hook's answer without changing it, and prints nothing | The runner and aggregator in `chunk-M6NSK26M.js`; runs with a recording hook and with the forwarder, which printed nothing and warned of nothing | Binary check, read and run |
| A timeout is in milliseconds, 60000 by default. At it the hook gets SIGTERM, the event counts it failed, and Gemini CLI shows "Hook(s) [kalareach] failed for event SessionStart. Press F12 to see the debug drawer for more details." The event ends only when the hook's standard streams close, so a child that keeps them open holds it | The runner in `chunk-M6NSK26M.js`; a hook that slept past a 1000 millisecond timeout: 1,116 milliseconds without a child and 12,253 with a background child still holding the streams | Binary check, read and run |
| The CLI waits for `SessionEnd` hooks at exit and on `/clear`, and an interactive exit runs them more than once, with the same session and reason: the main path registers them as an exit cleanup, and the interactive app both registers them and runs them when it unmounts. At that exit it prints three of its own lines about the hooks it ran | `runExitCleanup` in `chunk-GFHSA4BE.js`, the clear command in `chunk-GNCCSIBA.js`, the app container in `interactiveCli-COQJBPC2.js`; three interactive runs, each ended with Ctrl+C twice because the sign-in dialog took `/quit`: two `SessionEnd` hooks in an untrusted folder, and three in a trusted folder both with the install record and without it | Binary check, read and run |
| Unless `GEMINI_CLI_NO_RELAUNCH` or `SANDBOX` is set, the process that was started spawns a second node process that runs the session, and every hook's parent is that second process. The first one sizes the heap, adding `--max-old-space-size` at half the machine's memory where that is more than V8's limit and `advanced.autoConfigureMemory` is not false, starts the session again when it exits with code 199, as it does after an update or a restart it asks for, and relays administrator settings between the two | `bundle/gemini.js` and `getSpawnConfig` in `chunk-PJREBWY3.js`; runs recording each hook's parent and grandparent, with and without `GEMINI_CLI_NO_RELAUNCH=true` | Binary check, read and run |
| A directory under `extensions/` with no manifest in it is skipped, with a warning printed at every start | A run with an empty `extensions/kalareach/` | Binary check, run |

The package's own three files, the forwarder built from core and a stand-in worker listening where
a registration named it ran end to end, headless with no allow list and with one that matches the
record, and interactively: Gemini CLI ran the forwarder for `SessionStart` and `SessionEnd`, the
forwarder declared itself `{"application":"gemini-cli","surface":"hook"}` and reported
`thread_started` with detail `startup` and `thread_ended` with detail `exit` for Gemini CLI's own
session identifier. Headless, Gemini CLI printed nothing about either hook; interactively, only the
three lines at exit that the table above describes. With an allow list that does not match the
record, it printed the warning, ran the session and ran no hook. Status: binary check, run.

Why an extension and why these three events. An extension is the one place Gemini CLI takes hooks
from without sharing a key with the person's own, and its hooks run whether or not the folder is
trusted. `AfterTool` is left out because of the refusal the parsed standard error becomes: a
forwarder that is missing (bash exits 127 with "command not found") or too old to know this
invocation (it exits 64 with its usage) would replace every tool result the model reads. On the
three events registered, the worst such a failure can do is the warning above.

Limits, in the order they matter. Gemini CLI's relaunch means the process KalaReach launches never
starts a hook itself, so the worker records these observations and none of them selects the
session's thread; `GEMINI_CLI_NO_RELAUNCH=true` in the launch's environment would make the launched
process start them, and would also switch off the heap sizing and the restart after code 199, which
is the trade a command integration that sets it has to make. The recipe's removal steps delete the
three files, the record last, and cannot remove a directory or leave a file on a condition, so two
things fall to whatever applies them. It has to take the record only once nothing else is left in
`extensions/kalareach/`, keeping it beside a file of the person's or one that changed, and it has to
remove the directories the installation created once they are empty: under an allow list a directory
there without a record, empty or not, stops Gemini CLI from starting, and without an allow list an
empty one is warned about at every start. Where a person's settings list allowed extensions, the
hooks run only if the person adds a pattern that matches the record's source; nothing the recipe
writes can add one. The bridge range is `=0.60.0`, the only release checked. Not verified: a
`Notification` event and its payload on the binary, which need a tool that asks for permission and
so a model turn, and a session launched by a running KalaReach worker.

### What is not qualified here

Agent-protocol mode creates its own execution. Nothing here attaches it to a conversation already
running in a terminal. The vendor document that build ships at `bundle/docs/cli/acp-mode.md` (pinned
by digest above, read 2026-09-20) describes the mode as a client starting the agent over standard
streams for programmatic control, and names no way to join a session already running in a terminal.
Status: the mode's own behaviour is a binary check; the absence is a document check of that one
document, which is weaker than a test, and nothing here tested it.

Reverse filesystem and terminal requests must run in the selected host environment with resources
the broker scopes. The names are a binary check; the behaviour needs a live turn and is not observed
here.

`max_message_bytes` is 8 MiB, this host's bound. Prompt content can carry an image inline, which is
why the bound is not smaller. Volatile forwarding is untested.

No account was signed in to and no turn was started.

## kalareach/kimi-code-cli

### Identities

| What | Value | Source | Read | Status |
| --- | --- | --- | --- | --- |
| Executable | `kimi` | `kimi --version` on the qualification machine, reporting `2.0.2` | 2026-09-20 | Verified against a live install |
| Distribution | The kimi.com installer into `~/.kimi-code` | The installed tree, whose executable is `~/.kimi-code/bin/kimi` (sha256 `c2204148f56c872539ac37bfe1868b597adee3606a3d7698ee9b9687aa537f11`) and whose `config.toml` names `api.kimi.ai`; that build prints `https://moonshotai.github.io/kimi-code/` as its documentation | 2026-09-20 | Verified against a live install |
| Protocol | The agent protocol over standard streams, `kimi-code-cli-acp` | The handshake that build answers under its `acp` subcommand | 2026-09-20 | Binary check |
| Protocol version tested | 2.0.2 | `agentInfo` in that handshake: `{"name":"Kimi Code CLI","version":"2.0.2"}`, and `protocolVersion` 1 | 2026-09-20 | Binary check |
| Qualified range | `=2.0.2` | This qualification | 2026-09-20 | The only release probed |

### The two distributions

Two products share the name Kimi, and the qualification turns on telling them apart before any
protocol is chosen.

| | This package's profile | The other profile |
| --- | --- | --- |
| Home | `~/.kimi-code` | `~/.kimi` |
| Installed by | the kimi.com installer | MoonshotAI's own distribution |
| Reports itself as | `agentInfo.name` `Kimi Code CLI` | not observed here |
| Server surface | `kimi web`, a local REST and WebSocket server, and the vendor's remote control | an experimental Wire surface, not observed here |

The evidence that they are two: the installed build ships a `migrate` subcommand whose own help says
it migrates "data from a legacy kimi-cli installation into kimi-code". `~/.kimi` does not exist on
the qualification machine, so the other distribution is not installed and nothing here is a claim
about its wire format. Status: binary check for this distribution; the other is a separate profile,
qualified in its own package, `kalareach/kimi-cli`.

The match rule that names `~/.kimi-code/bin` is what identifies this one. A rule that recognises an
executable called `kimi` by its name alone is `inferred` and is presented as a guess: an executable
with that name establishes nothing about which server contract is available, and a directory of
saved sessions establishes nothing about which process owns the live one.

The refusal is declarative. A host whose installation evidence does not match the pins in this
package's record reports the capability as `incompatible`, and the case named in
`fixtures/conformance.json` states that both controls then disappear, leaving the terminal path
alone. `kalareach-catalogue validate` evaluates that case against this package's own predicates on
every run. Reading that evidence and comparing it against those pins is the host's half of the
contract; this package neither performs it nor tests it. A frame-level discriminator would need the
other distribution installed, and it is not here.

Native-TUI attachment to a server this package selected would need source or executable verification
this qualification does not have. Status: not claimed, and not offered.

### The declarative proxy contract

The agent protocol meets it, so this package ships no native bridge: JSON-RPC 2.0 over the started
process's standard streams, one document per line, identifiers at `id`, methods at `method`,
responses matched by repeating the `id`. Status: verified by sending requests to that build.

### Method classification

Twenty-two methods. Each host-to-upstream name was sent to the pinned build, and each
upstream-to-host name is present in that build's own executable.

| Sent | What 2.0.2 answered |
| --- | --- |
| `initialize` | A result: protocol version 1, `agentInfo`, prompt capabilities for image and embedded context, session capabilities for list, resume, close, delete, fork and additional directories, and one terminal login method |
| `session/list` | A result |
| `authenticate`, `session/load`, `session/prompt`, `session/set_mode`, `session/set_model`, `session/resume`, `session/close`, `session/delete`, `session/fork` | An error about the request's own parameters, which is the method answering |
| `session/new` | Authentication required, so the request reached the method and created nothing |
| `session/cancel` | Method not found as a request, which is what a notification-only handler does; not exercised as a notification |
| A name nobody implements | Method not found |

The nine upstream-to-host names are present in the installed executable. They are the requests the
agent makes of its client, so probing them from the client side reports method not found, which is
the correct answer rather than evidence of absence.

Status: a binary check for every name. The class beside each name is a judgement recorded as
evidence in the table itself and not exercised against a live turn.

### What is not qualified here

Nothing in this package depends on `kimi web`'s REST and WebSocket server or on the vendor's remote
control. Both are the vendor's own services, and neither is evidence that KalaReach may reuse a
service protocol. Status: out of scope, deliberately.

`max_message_bytes` is 8 MiB, this host's bound. Volatile forwarding is untested. No account was
signed in to and no turn was started; the probes ran with a home directory belonging to this
qualification, and `~/.kimi-code` was unchanged before and after.

## kalareach/qoder-cli

### Identities

| What | Value | Source | Read | Status |
| --- | --- | --- | --- | --- |
| Executable | `qoder`, dispatching to `qodercli` | `qodercli --version` of the build below, reporting `1.1.63`, run with a home directory belonging to this qualification | 2026-09-25 | Binary check |
| Distribution | The qoder.com installer into `~/.qoder` | The installer at `https://qoder.com/install`, read and not run, fetches the release manifest at `https://qoder-ide.oss-accelerate.aliyuncs.com/qodercli/channels/manifest.json`, downloads the archive the manifest names for the platform, checks it against the manifest's SHA-256, extracts `qodercli` and hands over to that executable's `install` subcommand, which places it under `~/.qoder`. The manifest named `1.1.63`, published 2026-09-24T10:02:00Z. This qualification took the first three steps into a directory of its own and ran no `install`: the darwin-arm64 archive matched the manifest, and the executable in it is byte-identical to the `~/.qoder/bin/qodercli/qodercli-1.1.63` the vendor's installer had put on this machine | 2026-09-25 | Verified against the vendor's release manifest |
| Protocol | The agent protocol over standard streams, `qoder-cli-acp` | The handshake that build answers under `--acp` | 2026-09-25 | Binary check |
| Protocol version tested | 1.1.63 | `agentInfo` in that handshake: `{"name":"qoder-cli","title":"Qoder CLI","version":"1.1.63"}`, and `protocolVersion` 1 | 2026-09-25 | Binary check |
| Qualified range | `=1.1.63` | This qualification | 2026-09-25 | The only release probed. Version 0.2 of this package, qualified against 1.1.59 on 2026-09-20, stays published as it was |

The vendor's installer also names an npm package, `@qoder-ai/qodercli`, for Windows, and the npm
registry publishes it, `latest` 1.1.63, with `qoder` and `qodercli` as its commands. Source:
`https://registry.npmjs.org/@qoder-ai/qodercli`, read 2026-09-25. Status: verified against the
registry. This qualification did not install or probe that distribution, so no rule here claims it
exactly: the rules name the two stable entry points wherever they are installed, the dispatcher
inside the directory the vendor installs it into, and the versioned executable of the qualified
release, and every one of them is `inferred`.

The executable carries its version in its file name, and `~/.local/bin/qodercli` is a link to it. A
match rule compares a whole file name and removes only a `.exe` suffix, so recognising
`qodercli-1.1.63` means naming that spelling. This package is qualified against 1.1.63 alone, so it
carries exactly that rule, narrowed to `~/.qoder/bin/qodercli`, beside the two rules for the stable
entry points: a host that resolves a launch all the way to the versioned file recognises the release
this package covers, and a host that stops at the dispatcher or the command name recognises the
application by that name. A release this qualification does not cover matches the stable names only,
which is what an `inferred` rule is for, and a package qualified against that release names its file
in its own rules. Status: the rule vocabulary compares file names rather than version-stripped
stems, recorded, and covered for the qualified release rather than worked around.

### The flag

`--acp` is not in 1.1.63's `--help`, as it was not in 1.1.59's. It works: the build answered an
initialize sent over its standard streams with it. Status: binary check, and the gap between the
help text and the behaviour is recorded rather than smoothed over.

### The declarative proxy contract

The agent protocol meets it: JSON-RPC 2.0 over the started process's standard streams, one document
per line, identifiers at `id`, methods at `method`, responses matched by repeating the `id`. Status:
verified by sending requests to that build.

### Method classification

Twenty-two methods, the same table as for 1.1.59, probed again on 1.1.63. Each host-to-upstream
name was sent to that build, and each upstream-to-host name is present in its own executable. Every
answer was the one 1.1.59 gave, and the three requests `fixtures/frames.json` pins as captured were
sent again, the working directory in the one that creates an execution aside, and answered the
same way.

| Sent | What 1.1.63 answered |
| --- | --- |
| `initialize` | A result: protocol version 1, `agentInfo`, prompt capabilities for image and embedded context, session capabilities for list, resume, close, delete, fork and additional directories, MCP capabilities for HTTP and SSE, one login method, and a vendor extension declaring prompt queueing |
| `authenticate`, `session/load`, `session/prompt`, `session/set_mode`, `session/set_model`, `session/resume`, `session/close`, `session/delete`, `session/fork` | An error about the request's own parameters, which is the method answering |
| `session/new`, `session/list` | Authentication required, so the request reached the method and created nothing |
| `session/cancel` | Method not found as a request, which is what a notification-only handler does; not exercised as a notification |
| A name nobody implements | Method not found |

The nine upstream-to-host names are present in the executable, and probing them from the client
side reports method not found, which is the correct answer rather than evidence of absence.

Status: a binary check for every name. The class beside each name is a judgement recorded as
evidence in the table itself and not exercised against a live turn.

### Hooks, and why this package installs none

Qoder CLI's hooks follow Claude Code's shape: a `hooks` object keyed by event, each value a list of
groups, each group a list of hooks, where a `command` hook with an `args` list runs in exec form, as
its own process with no shell, and one without runs through `bash -c`. A timeout is in seconds, 600
by default. On `SessionStart`, `SessionEnd`, `PostToolUse`, `PostToolUseFailure` and `Notification`
only exit code 2 refuses anything, the build reads standard output only from a hook that exits 0,
and it treats any other exit code as a failure it logs. Sources: the hooks reference at
`https://docs.qoder.com/cli/hooks` and the plugins reference at `https://docs.qoder.com/cli/plugins`,
read 2026-09-25, and the hook runner in the 1.1.63 executable. Status: document check and binary
check of the executable's own code.

Where a recipe could put KalaReach's hooks, each checked on 1.1.63 with a home directory belonging to
this qualification, the network denied and no account:

| Source | What it would take | Status |
| --- | --- | --- |
| `~/.qoder/settings.json` | A hooks key per event, which holds the person's own hooks for that event too: adding KalaReach's replaces theirs, and removing the key removes whatever they added to it later. There is one user-level settings file; the others are a project's, a project's local one and policy | Document check, and binary check of the settings sources the executable watches |
| An installed plugin | Qoder CLI loads an installed plugin from its install record in `~/.qoder/plugins/installed_plugins_v2.json`, which names the plugin's absolute path. `qodercli plugins install <dir>` wrote that record and `enabledPlugins`; the plugin's hooks ran with that absolute path and did not load with the same path written from `~/`. A recipe of fixed bytes cannot write a person's absolute path there | Binary check, run |
| A local marketplace | Two keys the settings file does not otherwise hold: `extraKnownMarketplaces.kalareach`, a directory source whose path is written with `${HOME}`, which the settings reader expands, and `enabledPlugins.kalareach-hooks@kalareach`; and three files, the marketplace's manifest and a plugin with its `hooks/hooks.json`. At the first start after the keys appear, Qoder CLI copies the marketplace into its own plugin state (`plugins/marketplaces/kalareach`, recorded in `plugins/known_marketplaces.json`) and runs none of its hooks; from the next start the plugin's hooks run beside the person's own, with no install record | Binary check, run over two sessions |
| The launch | `--settings` with inline JSON adds a settings source whose hooks run beside the person's own, and `--plugin-dir` loads a plugin directory for that run alone; both ran | Binary check, run |

So two sources add hooks without replacing the person's own: the launch, and a local marketplace.
KalaReach's hooks reach Qoder CLI on the launch, as `--settings` and inline JSON. Either way a
session gets its registration only when KalaReach launches it through a command integration, which
no package can declare yet, and a hook without one answers `{}` and reports nothing. The launch
writes nothing into the person's Qoder CLI directory; the marketplace would cost two keys in the
person's `settings.json`, Qoder CLI's own copy and record of the marketplace left behind after
removal, and no hooks in the first session after installation. The marketplace is the route to take
if a later Qoder CLI stops reading `--settings`.

The core repository defines the launch's two elements (`fixtures/bridges/qoder-cli/flags.json`,
sha256 `d867f03b41f63a11688ee1c6e0a79455ffbaca09d2c38150b6f8d4b4c269261f`) and the forwarder they
start, `kr-hook qoder-cli hook`, registered in exec form for the five events above with a timeout of
one second for `SessionEnd` and five for the rest. This package declares no native bridge and claims
none of those hooks' observations.

What the build did with them, each run with no account:

- In a folder already trusted, the interactive terminal fired `SessionStart` and `SessionEnd`, and
  ran the hooks from `--settings` and from the user's settings file beside each other. Each hook's
  parent was `qodercli` itself, the process that was started. The `qoder` dispatcher execs
  `qodercli` in place (read from `~/.qoder/entry/qoder`, sha256
  `07def4d998e91995000266cecabdedde71113307f122f9655a568c61369baf42`). Status: binary check, run.
- The core fixture's two elements, with the forwarder built from core and a stand-in worker, ran end
  to end: Qoder CLI ran `kr-hook qoder-cli hook` for `SessionStart` and `SessionEnd`, logged each as
  succeeding with exit code 0 in 33 milliseconds, and the forwarder declared itself
  `{"application":"qoder-cli","surface":"hook"}` and reported `thread_started` with detail `startup`
  and `thread_ended` with detail `other` for Qoder CLI's own session identifier. Status: binary
  check, run.
- Qoder CLI runs no hook from any source, the launch's included, in a folder the person has not
  trusted, and it fires a session's `SessionStart` before the person answers its trust question, so
  that start is blocked. Status: binary check, run.
- Headless print mode checks the account before it starts a session: with none, it fired only
  `SessionEnd`. Status: binary check, run.
- A hook that reaches its timeout is killed at once, with SIGKILL to its process group, and settled
  as timed out. Status: binary check of the executable's own code; not run.

Not verified: `PostToolUse`, `PostToolUseFailure` and `Notification` on the binary, which need a
model turn, and a session launched by a running KalaReach worker.

### What is not qualified here

Agent-protocol mode starts a subprocess with its own execution. Nothing in the installed build
offered a way to join a terminal that is already running, and no vendor document was read for this
package, so this package offers none. Status: the mode's own behaviour is a binary check; the
absence of an attachment path is unverified and recorded as a boundary rather than as a document
check.

The vendor's remote-control feature runs through the vendor's own application and account. It is not
evidence that KalaReach may reuse that service protocol, and nothing in this package depends on it.
Status: out of scope, deliberately.

`max_message_bytes` is 8 MiB, this host's bound. Volatile forwarding is untested. No account was
signed in to and no turn was started. The 1.1.63 probes ran a copy in a directory of this
qualification's own, with a home directory of its own. Files under `~/.qoder` did change while they
ran, and each belongs to one of three runs no probe started, which Qoder CLI recorded in its own run
manifests: the installed `qodercli` run as `--version`, `update` and `--version` from the home
directory, the same three this machine runs about every six hours.

## kalareach/opencode-attach

### Identities

| What | Value | Source | Read | Status |
| --- | --- | --- | --- | --- |
| Executable | `opencode` | `opencode --version` on the qualification machine, reporting `1.18.31`; the native executable is the one pinned by digest above | 2026-09-24 | Binary check |
| Distribution | npm `opencode-ai` | `https://registry.npmjs.org/opencode-ai`, the 1.18.31 release, installed into a directory owned by this qualification and nowhere else | 2026-09-24 | Binary check of the installed release |
| Protocol | The earlier API family's event stream, `opencode-server-events` | The OpenAPI document that build serves at `GET /doc`, fetched again for this qualification and byte for byte the document pinned above | 2026-09-24 | Binary check |
| Protocol version tested | 1.18.31 | The same document | 2026-09-24 | Binary check |
| Qualified range | `=1.18.31` | This qualification | 2026-09-24 | The only release whose schema was read |
| Terminal route | `opencode serve`, then `opencode attach <url>` | That build's own help, which lists both: one starts a headless server and the other attaches the terminal to a running one | 2026-09-24 | Binary check |

### The declarative proxy contract

The event stream meets it, so the package ships no native bridge for that leg.

`GET /event` is a server-sent event stream whose every message is one JSON document with exactly
three members, `id`, `type` and `properties`, all of them required. `id` is a string, and every
variant but `server.instance.disposed` requires it to match `^evt_`; that one requires a string and
nothing more, so an identifier without the prefix is still a valid event of this family. That gives
the table a framing and a method name at `type`. The identifier at `id` is the durable event
identifier the stream orders by; nothing travels host to upstream on this leg, so there is no
request here for a response to answer, and the correlation the manifest requires repeats the same
path. That is the whole of what correlation means here. The route also takes optional `directory`
and `workspace` query parameters, which the document names without describing; choosing them is the
host's.

The request leg does not meet it, for the reason recorded for `kalareach/opencode`: asking the
server to do something is an HTTP request whose method is a verb and a path, such as
`POST /session/{sessionID}/permissions/{permissionID}`, and `Framing` has no HTTP member. Status:
named here as outside the table. Interrupting a turn goes through the broker's own cancellation
against the bound execution.

### The two API families

This is the other side of the comparison in the `kalareach/opencode` section, read from the same
document.

| | This package's profile | The other profile |
| --- | --- | --- |
| Event stream | `GET /event`, schema `Event` | `GET /api/event`, schema `V2Event` |
| Envelope | `id`, `type` and `properties`, and nothing else | `id`, `type` and `data`, and optionally `durable`, `location` and `metadata` |
| Operation identifiers | unprefixed | prefixed `v2.` |
| Answering a permission | `POST /session/{sessionID}/permissions/{permissionID}` or `POST /permission/{requestID}/reply` | `POST /api/session/{sessionID}/permission/{requestID}/reply` |

The `Event` union carries 89 names. The `V2Event` union carries 88 of them and nothing else, so no
name belongs to the shared-server family alone, and this table has none it could refuse by name.
What it has instead is the one name only its own family publishes, `server.instance.disposed`, which
it reads as the observation it is while the `kalareach/opencode` table refuses the same name. A
frame from the shared-server family routes by its shared name here and carries nothing under
`properties`, so the table extracts nothing from it. `fixtures/frames.json` pins one envelope of
each kind; `a_frame_from_the_other_api_family_is_unsupported_or_carries_no_payload` in
`pipeline/tests/packages.rs` asserts that the shared-server envelope carries no payload this table
reads, and `a_name_only_one_api_family_publishes_is_read_by_its_own_table_and_refused_by_the_other`
asserts both tables' answers for the exclusive name.

Which family a connection belongs to is settled before it opens, from the installed version and the
served schema, and a host whose evidence does not match this package's pins reports the capability
as `incompatible`. The case in `fixtures/conformance.json` states what this package draws then:
nothing, so the terminal path is what is left. Reading the installed evidence and comparing it
against those pins is the host's half of that contract. This package neither performs it nor tests
it, and nothing here records it as done.

Every pinned frame was checked against the document's own unions with a JSON Schema 2020-12
validator, by a script kept outside the repository: the eight frames of this family validate against
`Event` and not against `V2Event`, and the shared-server envelope validates against `V2Event` and
not against `Event`. Status: binary check.

### Method classification

Eighty-nine event names: every name in that document's `Event` union, with nothing added and nothing
left out. The eighty-eight names both families publish carry the classes of the `kalareach/opencode`
table, and the evidence beside each changes only where it named a family. `server.instance.disposed`
reports that the server instance for a directory was disposed, and it is an observation here.

Four are not observations, for the reasons recorded for `kalareach/opencode`: `tui.session.select`
and `tui.toast.show` are mutations, and `tui.prompt.append` and `tui.command.execute` are declared
unsupported, because forwarding either would be automatic composer insertion or a typed control
surface over somebody's terminal. No event on this stream carries a credential.

Status: the names and their payload shapes are a binary check of the served document. The class
beside each name is a judgement argued from the documented payload and recorded as evidence in the
table itself; no classification was exercised against a running turn, because that needs a vendor
account.

### What is not qualified here

`max_message_bytes` is 8 MiB, this host's bound. Volatile forwarding is untested.

Nothing was driven: no turn was started, no permission was answered, no terminal was attached, and
no account was signed in to. The server was started once, on loopback, with a home directory and a
working directory belonging to this qualification, only to read the document it serves, and it was
stopped by the process identifier recorded when it started.

## kalareach/kimi-cli

### Identities

| What | Value | Source | Read | Status |
| --- | --- | --- | --- | --- |
| Executable | `kimi`, and `kimi-cli` beside it | `kimi --version` on the qualification machine, reporting `kimi, version 1.51.0`; the installed package's entry points name both as console scripts for the same function | 2026-09-24 | Binary check |
| Distribution | PyPI `kimi-cli`, MoonshotAI's own | The index entry at `https://pypi.org/pypi/kimi-cli/json` names `https://github.com/MoonshotAI/kimi-cli` as its source, and that repository names `kimi-cli` on PyPI as its package. The 1.51.0 wheel, pinned by digest above, was installed into an environment owned by this qualification | 2026-09-24 | Document check of the identity; binary check of the installed wheel |
| Home | `~/.kimi` | The installed package's own share directory, which a `KIMI_SHARE_DIR` variable can move | 2026-09-24 | Binary check |
| Protocol | The agent protocol over standard streams, `kimi-cli-acp` | The handshake that build answers under its `acp` subcommand | 2026-09-24 | Binary check |
| Protocol version tested | 1.51.0 | `agentInfo` in that handshake: `{"name":"Kimi Code CLI","version":"1.51.0"}`, and `protocolVersion` 1. The protocol library it installs is agent-client-protocol 0.8.0, pinned by digest above | 2026-09-24 | Binary check |
| Qualified range | `=1.51.0` | This qualification | 2026-09-24 | The only release probed |

### Which release

MoonshotAI has archived kimi-cli. The repository and the index entry both say it is no longer
maintained and name the kimi.com distribution as its replacement. Source:
`https://github.com/MoonshotAI/kimi-cli` and its changelog, read 2026-09-24. Status: document check.

The final release, 1.52.0, carries no agent protocol. Started with no arguments, its entry point
takes an install command from the migration metadata it fetches from the vendor, from a cached copy
of that metadata, or from a built-in default that downloads the kimi.com installer with `curl` and
pipes it to a shell, and runs that command without asking; given `--version` it prints the version
and a deprecation notice; given anything else it prints the notice alone. That was read from the
1.52.0 wheel, pinned by digest above, which was not installed or run. Status: document check of the
published wheel. The practical consequence is outside this package: a host that starts a 1.52.0
installation on the terminal route with no arguments runs that install command, whatever profile it
selected.

1.51.0 is the newest release that runs the agent itself, so it is the one pinned. The Python sources
of 1.50.0 differ from it only in the build identifier, which was read from both wheels; 1.50.0 was
not probed, and the qualified range does not admit it.

### The two distributions

Two products share the executable name `kimi`, and the qualification turns on telling them apart
before any protocol is chosen.

| | This package's profile | The other profile |
| --- | --- | --- |
| Home | `~/.kimi` | `~/.kimi-code` |
| Installed by | PyPI `kimi-cli` | the kimi.com installer |
| Executables | `kimi` and `kimi-cli` | `kimi` |
| `agentInfo` in the handshake | `Kimi Code CLI`, 1.51.0 | `Kimi Code CLI`, 2.0.2 |
| Session capabilities advertised | list, resume | list, resume, close, delete, fork, additional directories |
| Other surfaces | `kimi --wire`, the Wire protocol; `kimi web` | `kimi web`, a local REST and WebSocket server, and the vendor's remote control |

The other profile's column is the `kalareach/kimi-code-cli` section's evidence, read 2026-09-20 from
the installed kimi.com build; this column was read from 1.51.0.

Both builds answer the handshake with the same agent name, so the name tells nobody which one
answered, and no method name does either: both speak the standard agent-protocol method set. What
does tell them apart is in the same answer: the version, and the session operations each advertises.
Each package pins its own build's answer, and
`the_kimi_distributions_share_an_agent_name_and_differ_in_what_their_handshake_advertises` in
`pipeline/tests/packages.rs` asserts that the names are equal, that the versions and the advertised
operations differ, and that each table routes exactly the session operations its own build
advertises, refusing or leaving unrouted the rest. Status: binary check on both sides.

The rules that name the PyPI project identify this distribution. A rule that recognises an
executable called `kimi` or `kimi-cli` by name alone is `inferred` and is presented as a guess: the
name `kimi` is shared with the kimi.com build, and neither name establishes which server contract is
available. The refusal is declarative, as it is for `kalareach/kimi-code-cli`: a host whose
installation evidence does not match the pins in this package's record reports the capability as
`incompatible`, and the case in `fixtures/conformance.json` states that both controls then
disappear. Comparing that evidence, including the handshake above, against the pins is the host's
half of the contract; this package neither performs it nor tests it.

Native-TUI attachment to a server this package selected would need source or executable verification
this qualification does not have. Status: not claimed, and not offered.

### The declarative proxy contract

The agent protocol meets it, so this package ships no native bridge: JSON-RPC 2.0 over the started
process's standard streams, one document per line, identifiers at `id`, methods at `method`,
responses matched by repeating the `id`. Status: binary check by sending requests to that build.

The `--acp` flag, which the build's own help marks deprecated, answered `initialize` with invalid
params and a message naming the `acp` subcommand in its place, and a name nobody implements with
method not found. Its source raises the same error from every agent-protocol method it has. Status:
binary check of the two answers; the rest is read from the installed source.

### Method classification

Twenty methods. Each host-to-upstream name was sent to the pinned build once, and each
upstream-to-host name is one the installed package calls, through the method table of the protocol
library it installs.

| Sent | What 1.51.0 answered |
| --- | --- |
| `initialize` | A result: protocol version 1, `agentInfo`, prompt capabilities for image and embedded context, session capabilities for list and resume, MCP over HTTP and not over server-sent events, and one terminal login method |
| `session/list` | A result, with no sessions |
| `session/load`, `session/prompt`, `session/set_mode`, `session/set_model`, `session/resume` | An error about the request's own parameters, which is the method answering |
| `session/new` | Authentication required, so the request reached the method and created nothing |
| `authenticate` | Nothing. The build's log records that the handler ran, found no login, and failed to encode its own error, so no answer was sent |
| `session/fork` | An error about the parameters of an empty request, and an internal error for a well-formed one, because its handler is not implemented |
| `session/cancel` | Method not found as a request. Sent as a notification it drew no answer, as a notification should, and the log records that the handler received it |
| `session/close`, `session/delete`, `session/set_config_option` | Method not found |
| A name nobody implements | Method not found |

A method the build answered method not found is not routed, so `session/close` and `session/delete`,
which the kimi.com build implements, fall to the mutation default here. `session/fork` is routed and
declared `unsupported`: the build accepts the name and cannot carry it out, and its handshake does
not advertise forking. `session/set_model` is a mutation that also saves the chosen model as the
default in the agent's own configuration file, which was read from the installed source rather than
exercised. `authenticate` sending no answer is recorded so that nobody builds a host that waits for
one.

The nine upstream-to-host names are the requests the agent makes of its client, so probing them from
the client side reports method not found, which is the correct answer rather than evidence of
absence.

Status: a binary check for every name. The class beside each name is a judgement recorded as
evidence in the table itself and not exercised against a live turn.

### The Wire surface

`kimi --wire` is a second protocol on the same streams, marked experimental in the build's own help.
Sent read-only requests, 1.51.0 answered `initialize` with Wire protocol 1.10, the server name and
version above, its slash commands and the hook events it supports; answered `replay` with no events;
answered an unknown name with method not found; and answered an agent-protocol `initialize` with
invalid params. One package carries one table, and this package's table is the agent protocol's, so
nothing here depends on the Wire surface. Source: those requests, and the vendor's Wire document at
`https://moonshotai.github.io/kimi-cli/en/customization/wire-mode.html`, read 2026-09-24. Status:
binary check, and not covered by the table.

### What is not qualified here

Nothing in this package depends on `kimi web`, `kimi term` or the Wire surface.

`max_message_bytes` is 8 MiB, this host's bound. Volatile forwarding is untested. No account was
signed in to and no turn was started. Every probe ran with a home directory and a working directory
belonging to this qualification, telemetry switched off, the keyring backend disabled, and HTTP
proxies pointed at a closed loopback port, and the build answered every request above that way.
`~/.kimi` did not exist before or after, and a listing of `~/.kimi-code` with every entry's
modification time and size hashed to the same value before and after.
