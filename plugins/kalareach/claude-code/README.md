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
belongs to the pending request the host is holding. Text that is not a decision for a pending request
is not an answer, and it does not become an ordinary message to Claude either: the message path is a
separate action with its own effect class, and the two never fall through into one another.

This package interprets nothing. It ships no decoder, and the table routes and classifies rather than
reading a request's fields, so the approval a person answers is the one the host already holds in its
ledger.

That is also why this package draws no Allow and Deny buttons of its own. A document it ships is
written before any request exists, and the only fact it could test is that some approval is pending,
which does not name one. The answer belongs beside the approval the host is holding, and the action
registered here is what encodes it.

Claude Code applies whichever answer reaches it first, the terminal's or this one, and drops the
other. It does not tell the channel which happened, so a delivered answer is evidence that the
answer was sent and nothing more.

Delivering a message is not steering and not an acknowledgement that anything was processed. Claude
Code queues messages that arrive while a turn is running and delivers them together on the next turn,
and the write to the transport is the only receipt there is.

## What stays in the terminal

Project trust and MCP server consent. Claude Code asks for both in its own terminal and relays
neither, so this package offers no control for them, whatever rights the person holds.

## Runtime gates

The registration is what makes the channel available; it is not what turns it on. Claude Code
registers a channel only when every one of these holds where it runs:

- The session names the server or its plugin at launch. A registration Claude Code can see is not a
  registration it uses.
- The plugin is on the effective allowlist, which is the vendor's own unless an organisation replaces
  it, or the session was started with the development flag instead.
- The organisation's channel setting permits channels at all.
- The session authenticates in a way that supports channels, which rules out the third-party model
  providers.
- The server declares the channel capability, and the permission capability as well before any
  approval is relayed to it.
- The negotiated protocol revision is one this version of Claude Code registers a channel over.
- The feature itself reaches this installation. It is a research preview, rolling out gradually, and
  a session it has not reached refuses the registration whatever the settings say.

Each of those is decided where Claude Code runs, not here. Until they all hold, the approval is
answered in the terminal.

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
`Notification`. Those five were chosen because none of them refuses what Claude Code was about to do
when a handler exits with a failure code. That narrows the ways a hook can interfere; it does not
remove them, because a handler that answers with a stop decision stops the session on any event.
What makes these hooks observers is that the forwarder answers nothing: it writes the event to the
worker and exits. Each registration also carries its own timeout, so a forwarder that cannot reach
the worker costs seconds rather than the ten minutes a command hook may otherwise take.

The registration installs disabled. Claude Code loads a plugin from a skills directory as soon as it
is there, so the manifest sets its default to off and the settings key is what turns it on. Removal
takes that key out first, which is what stops the registration loading, and then deletes the three
files in the reverse of the order it wrote them. Each file is checked against the digest it was
installed with, and one that somebody has since edited is reported and left alone rather than
deleted.

The forwarder runs under Claude Code's own permissions, outside the KalaReach plugin sandbox and
outside Wasmtime, because Claude Code is the process that starts it. The installation grant says
exactly that, and it says what the forwarder can see. None of the three files carries a session
identifier or a secret: what the registration is worth is decided by the forwarder binding it to the
launch KalaReach made and to the worker's private exchange, and the forwarder is the host's, not
this package's.

## What it asks for

Five capabilities. Matching, declarative presentation and broker semantic events are the reading
half. The upstream action capability carries a message you wrote. The approval capability answers one
pending tool call, through the action registered here. The bridge installation grant is declared with
the recipe it installs.

## Fixtures

`fixtures/conformance.json` states what a person sees in six situations, and the build evaluates
this package's own predicates against each one: a bound session, an actor who may only view,
volatile-native operation, a session with no qualified evidence for acting upstream, an upload in
progress, and a binding disabled by repeated faults. They are display conformance: the fixture runner
evaluates control predicates and never reads a channel frame.

Some of what this package promises is structural rather than conditional, and a fixture cannot state
it. There is no action for project trust or MCP consent, so no right produces a control for either.
There is no action that steers a running turn, so a message delivered to a busy session cannot be
presented as steering.

`docs/qualification-notes.md` in this repository records what every claim above was checked against,
and which claims were not checked against a live install.
