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
| Codex generated schema, `codex_app_server_protocol.schemas.json` | | `f1f3591667d8dcf7` (first 16) |
| Codex generated schema, `codex_app_server_protocol.v2.schemas.json` | | `f0402dc8ce8d2781` (first 16) |
| Claude Code 2.1.278 executable | macOS, arm64 | `bd245662fb8a0e32` (first 16) |

The App Server is the native executable, which is what the schema above was generated from. The
launcher is listed beside it because it is what npm puts on the path.

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
result, are protocol behaviours. This repository's fixture format evaluates control predicates
against facts the host supplies and reads no vendor frame, so it cannot express any of them. What
the package contributes to each is stated in its README: a routing and a classification, or in two
cases nothing at all. Exercising them needs a frame corpus and a driver, and both belong to the
gateway. Status: not covered here, and the requirement row stays open for that half.

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
unverified against a live install, because it depends on the forwarder, which is core's.

The table routes and classifies. It carries no mapping from a relayed permission request's
`tool_name`, `description` and `input_preview` onto an approval resource a person can read, because
the connector manifest has no field for such a mapping, and this package ships no component and so
no decoder. The package supplies the answer, and the resource it answers is the one the host already
holds: the approve and deny controls are gated on the ledger's pending-approval fact. A package's
own document cannot carry an `approval_ref`, since that node names a runtime resource. Status: the
answer path verified against the published documentation; the resource mapping is not this package's
and is recorded as an open interface question rather than claimed.

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

The package registers no answer action. The identifier is the field the broker owns, so an answer's
identifier comes from the request being resolved rather than from a caller. The vendor's own documented failure mode is the one this is
meant to close: a reply in the wrong format falls through to Claude as an ordinary message, and a
reply naming an identifier nobody issued is dropped in silence. Inside KalaReach neither becomes a
message, because the message path is a separate action with its own effect class. Status: the
notification shapes are a document check. The path an answer takes inside the host is not this
package's and is not verified here.

The package draws no Allow and Deny control and registers no answer action. A control's visibility is
a predicate over facts the host knows, and the only fact available is that some approval is pending,
which does not name one; a control drawn on that fact could be answered against a request that became
pending after the person read a different one. The SDK's plugin-action invocation carries no
reference to an approval resource either, while the host's own approval method does. So this package
supplies the table and asks for the trust to have an answer encoded through it, and the answer is
dispatched by the path that names the resource. The SDK has no parameter kind or control field that
names an approval resource, and a package document cannot carry an `approval_ref` because that node
names a runtime resource. Status: an interface limit, recorded rather than worked around.

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
can claim, which is why the answer controls are hidden until the host has evidence for them.

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
`{"continue": false}` stops the session whatever the event. A hook here observes because the
forwarder answers nothing, not because the event cannot carry an answer. Each registration also
carries a `timeout`, one second for `SessionEnd` and five for the rest, against a documented command
default of 600 seconds. Source: `https://code.claude.com/docs/en/hooks`, read 2026-09-20. Status:
document check.

Three things are unverified, and all three are recorded rather than assumed:

- The recipe was not installed into a live Claude Code and no session was started against it. Status:
  unverified. Installing it would change the qualification machine's own configuration, which a
  packaging run does not do. What removal does when a file's digest no longer matches, and whether
  Claude Code picks the registration up without a restart, are unverified for the same reason.
- The forwarder is named `kr-hook` and invoked as `kr-hook claude-code channel` and `kr-hook
  claude-code hook`. Those argument vectors are what this package requires of a host interface, and
  they were checked against no host. The same applies to everything else the registration depends on:
  the framing on the private exchange, the MCP negotiation and capability declarations the channel
  server makes, what the forwarder returns on a hook, and how it binds a registration to a launch.
  Status: unverified, and a requirement this package states rather than a behaviour it observed.
- Registration authentication is a requirement here, not an observation. None of the three installed
  files carries a session identifier or a secret, which can be checked by reading them, and that much
  is verified. That the registration is bound to the launch KalaReach made and to the worker's
  private exchange is what the forwarder must do, and it was not observed.
