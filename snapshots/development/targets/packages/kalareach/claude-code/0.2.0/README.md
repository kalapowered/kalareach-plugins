# kalareach/claude-code

Recognises Claude Code, observes it through lifecycle and tool hooks, and answers the tool call it
is waiting on.

## What it does

KalaReach matches the `claude` executable, labels the session, and keeps Claude Code in the terminal
KalaReach already owns. Nothing here replaces that terminal or starts a second Claude Code beside it.

Two surfaces reach the session, and they stay apart on purpose.

The hooks only watch. The installed registration points Claude Code's lifecycle and tool events at
the core forwarder, which carries them to the worker that owns the terminal. They report what
happened. They never return a decision that would change what Claude Code permits.

Channels carry messages in and the answer to a tool approval back. Claude Code spawns a channel as
an MCP server of its own and talks to it over that process's standard streams, so the gateway has
nowhere to sit between them. This package installs the registration that makes the core forwarder
that channel, and `connector.json` states how the frames on the private exchange are read.

## How an approval is answered

Claude Code relays a pending tool approval as `notifications/claude/channel/permission_request`, with
a five-letter `request_id`, the tool's name, a description of the call and a preview of its
arguments. The answer is `notifications/claude/channel/permission`, carrying that same `request_id`
and `allow` or `deny`.

The broker owns the identifier. `connector.json` puts the correlation at `params.request_id`, and the
answer action binds one parameter: the decision. A control cannot name a request, so an answer always
belongs to the pending request the person read. Text that is not a decision for a pending request is
not an answer, and it does not become an ordinary message to Claude either: the message path is a
separate action with its own effect class, and the two never fall through into one another.

Local and remote answers race, and the first one to arrive wins. Whoever answers second is told the
request is already resolved.

Delivering a message is not steering and not an acknowledgement that anything was processed. Claude
Code queues messages that arrive while a turn is running and delivers them together on the next turn,
and the write to the transport is the only receipt there is.

## What stays in the terminal

Project trust and MCP server consent. Claude Code asks for both in its own terminal and relays
neither, so this package offers no control for them, whatever rights the person holds.

## Runtime gates

The registration is what makes the channel available; it is not what turns it on. Claude Code
registers a channel only when the session names it at launch, when the plugin is on the effective
allowlist, and when the organisation's channel setting permits channels at all. Each of those is
decided where Claude Code runs, not here. Until all three hold, the answer controls stay hidden and
the approval is answered in the terminal.

## The registration it installs

Claude Code reads its channels and its hooks from files in its own directory, so the bridge is three
declarative files and one settings key. Nothing here is a program, and nothing here carries protocol
logic: each file names the core forwarder and the surface it should carry.

| Installed | What it is |
| --- | --- |
| `skills/kalareach-channels/.claude-plugin/plugin.json` | The plugin manifest, declaring one channel bound to the server below |
| `skills/kalareach-channels/.mcp.json` | The channel server: the core forwarder, started over standard streams |
| `skills/kalareach-channels/hooks/hooks.json` | Five hook registrations, all of them on events that cannot block |

The settings key is `enabledPlugins."kalareach-channels@skills-dir"`, set to `true`. The recipe adds
that one key and leaves every other setting where it was.

The hooks are registered on `SessionStart`, `SessionEnd`, `PostToolUse`, `PostToolUseFailure` and
`Notification`. Claude Code cannot be stopped by a handler on any of those five, whatever the handler
returns, so the observation stays observation by construction rather than by promise. The forwarder
writes the event to the worker and returns, so no hook holds the session open waiting for anybody.

Removal takes the settings key out first, so Claude Code stops loading the registration, and then
deletes the three files in the reverse of the order it wrote them. Each file is checked against the
digest it was installed with, and one that somebody has since edited is reported and left alone
rather than deleted.

The forwarder runs under Claude Code's own permissions, outside the KalaReach plugin sandbox, because
Claude Code is the process that starts it. The installation grant says exactly that, and it says what
the forwarder can see. It also authenticates: the registration is bound to the launch KalaReach made
and to the private exchange the worker opened, not to a session identifier taken from the
environment.

## What it asks for

Five capabilities. Matching, declarative presentation and broker semantic events are the reading
half. The upstream action capability carries a message you wrote. The approval capability answers one
pending tool call. The bridge installation grant is declared with the recipe it installs, and it says
that the registered forwarder runs under Claude Code's own permissions, outside the sandbox.

No component ships here, so nothing in this package interprets bytes on its own: the approval a
person sees is built from the fields Claude Code sends, read by core code through the table above.

## Fixtures

`fixtures/conformance.json` states what a person sees in nine situations, and the build evaluates
this package's own predicates against each one: a pending approval, no pending request, an actor
without the right to answer, a session whose channel is not registered, a message queued behind a
running turn, volatile-native operation, an actor holding host rights and still getting no trust
control, an upload in progress, and a binding disabled by repeated faults.
