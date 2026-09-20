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

## kalareach/codex

### Identities

| What | Value | Source | Read | Status |
| --- | --- | --- | --- | --- |
| Executable | `codex` | `codex --version` on the qualification machine, reporting `codex-cli 0.155.1` | 2026-09-20 | Verified against a live install |
| Distribution | npm `@openai/codex` | `https://registry.npmjs.org/@openai/codex`, `dist-tags.latest` = `0.155.1` | 2026-09-20 | Verified against the registry |
| Protocol | Codex App Server, `codex-app-server` | `https://learn.chatgpt.com/docs/app-server` | 2026-09-20 | Verified against the published documentation |
| Protocol version tested | 0.155.1 | The JSON Schema written by `codex app-server generate-json-schema`, whose v2 document is titled `CodexAppServerProtocolV2` | 2026-09-20 | Verified against a live install |
| Qualified range | `>=0.155.0, <0.156.0` | This qualification | 2026-09-20 | Narrowed to the release the schema was generated from; the transport and the protocol are both documented as experimental, so the range covers no release that was not read |

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

Every wire name in `connector.json` was taken from the generated schema rather than from prose: the
client requests from the `ClientRequest` union, the notifications from `ServerNotification`, and the
reverse requests from `ServerRequest`. A name not in that schema is not in the table.

Status: verified against a live install.

The class beside each name is a judgement about what the method does, argued from the documented
behaviour and recorded as evidence in the table itself. Those judgements were not exercised against
a running App Server. Status: unverified, because qualifying a classification by running it means
starting turns and approving commands against a real account, which is not something a packaging
run should do.

### What is not qualified here

The native `--remote` terminal connection speaks WebSocket, over TCP or over a Unix socket with the
standard HTTP upgrade handshake, rather than the line-delimited JSON the table declares. The table
describes the leg between KalaReach and the App Server. The leg between the native terminal and the
gateway is the gateway's own listener, and its framing is not expressible in a connector table
today. Status: unverified, and named here so nobody reads the table as covering both legs.

Volatile forwarding is declared as untested, which is what `volatile_forwarding: false` means. Until
it is tested, the unchanged terminal integration is the supported path when receipt storage fails.

## kalareach/claude-code

### Identities

| What | Value | Source | Read | Status |
| --- | --- | --- | --- | --- |
| Executable | `claude` | `claude --version` on the qualification machine, reporting `2.1.278 (Claude Code)` | 2026-09-20 | Verified against a live install |
| Distribution | npm `@anthropic-ai/claude-code` | `https://registry.npmjs.org/@anthropic-ai/claude-code`, `dist-tags.latest` = `2.1.278` | 2026-09-20 | Verified against the registry |
| Protocol | Claude Code Channels, `claude-code-channels` | `https://code.claude.com/docs/en/channels-reference` and `https://code.claude.com/docs/en/channels` | 2026-09-20 | Verified against the published documentation |
| Protocol version tested | 2.1.278 | The three channel notification names are present in the installed 2.1.278 executable | 2026-09-20 | Verified against a live install |
| Qualified range | `>=2.1.234, <3.0.0` | The version notes in the Channels reference | 2026-09-20 | 2.1.234 is the release that sends permission requests only to servers it registered as channels, treats an undeclared permission capability as undeclared, and masks credentials in the relayed fields. Below it the surface behaves differently enough that the table would be wrong |

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

### Method classification

Three methods, all of them JSON-RPC notifications:

- `notifications/claude/channel` delivers a message into the session.
- `notifications/claude/channel/permission_request` relays a pending tool approval out, with
  `request_id`, `tool_name`, `description` and `input_preview`.
- `notifications/claude/channel/permission` answers one, with `request_id` and `behavior`.

All three names are present in the installed 2.1.278 executable, as is the `request_id` field and
the five-letter identifier alphabet the documentation describes. Status: verified against a live
install for the names, and against the published documentation for the field shapes.

Because all three are notifications, none of them carries a JSON-RPC `id`. The correlation
identifier is `params.request_id`, which is where the table puts it. Status: verified against the
published documentation.

The answer action binds one parameter, the decision. It does not bind the identifier, because the
identifier is the field the broker owns, so an answer always names the request that is actually
pending. The vendor's own documented failure mode is the one this closes: a reply in the wrong
format falls through to Claude as an ordinary message, and a reply naming an identifier nobody
issued is dropped in silence. Inside KalaReach neither becomes a message, because the message path
is a separate action with its own effect class.

### Runtime installation gates

Registering the channel does not enable it. Three conditions are decided where Claude Code runs:

1. The session names the plugin at launch, with `--channels` or the development flag. Being in the
   MCP configuration is not enough.
2. The plugin is on the effective allowlist, which is Anthropic's list unless an organisation
   replaces it with `allowedChannelPlugins`.
3. The organisation's `channelsEnabled` setting permits channels at all.

Source: `https://code.claude.com/docs/en/channels`, read 2026-09-20. Status: verified against the
published documentation. Whether any of the three holds on a given machine is not something this
package can claim, which is why the answer controls are hidden until the host has evidence for them.

### The bridge, and what is unverified about it

The recipe installs three files into Claude Code's own directory and adds one settings key:

| Installed | Documented location | Source | Status |
| --- | --- | --- | --- |
| Plugin manifest | `.claude-plugin/plugin.json` under a skills directory, loaded in place with no marketplace | `https://code.claude.com/docs/en/plugins-reference` | Verified against the published documentation |
| Channel server | `.mcp.json` in the plugin root | `https://code.claude.com/docs/en/plugins-reference` | Verified against the published documentation |
| Hooks | `hooks/hooks.json` in the plugin root | `https://code.claude.com/docs/en/plugins-reference` | Verified against the published documentation |
| `enabledPlugins."kalareach-channels@skills-dir"` | `settings.json` under the user's Claude Code directory | `https://code.claude.com/docs/en/plugins-reference` and `https://code.claude.com/docs/en/hooks` | Verified against the published documentation |

Each of the three files uses its own documented location rather than the inline form the manifest
also accepts, because the documentation does not fix the inline shape and a registration that loads
is worth more than a shorter one.

The five hook events are `SessionStart`, `SessionEnd`, `PostToolUse`, `PostToolUseFailure` and
`Notification`. The hooks reference lists, per event, whether a handler can block; none of those five
can. Source: `https://code.claude.com/docs/en/hooks`, read 2026-09-20. Status: verified against the
published documentation.

Two things are unverified, and both are recorded rather than assumed:

- The recipe was not installed into a live Claude Code and no session was started against it. Status:
  unverified. Installing it would change the qualification machine's own configuration, which a
  packaging run does not do.
- The forwarder is named `kr-hook` and invoked as `kr-hook claude-code channel` and `kr-hook
  claude-code hook`. That argument vector is this package's assumption about an interface the core
  supplies. Status: unverified.
