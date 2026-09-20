# kalareach/claude-code

Recognises Claude Code, observes it through lifecycle and tool hooks, and carries a message into
the session.

## What it does

KalaReach matches the `claude` executable, labels the session, and keeps Claude Code in the terminal
KalaReach already owns. Nothing here replaces that terminal or starts a second Claude Code beside
it.

Two surfaces reach the session, and they stay apart on purpose.

The hooks watch. The installed registration points Claude Code's lifecycle and tool events at the
core forwarder, which carries them to the worker that owns the terminal. What the package asks of the
forwarder is that it answer nothing: write the event to the worker and exit, so no hook changes what
Claude Code permits.

Channels carry messages in and the answer to a tool approval back. Claude Code spawns a channel as
an MCP server of its own and talks to it over that process's standard streams, so the gateway has
nowhere to sit between them. This package installs the registration that makes the core forwarder
that channel, and `connector.json` states how the frames on the private exchange are read.

## What the table says about an approval

Claude Code relays a pending tool approval as `notifications/claude/channel/permission_request`,
with
a five-letter `request_id`, the tool's name, a description of the call and a preview of its
arguments. The answer is `notifications/claude/channel/permission`, carrying that same `request_id`
and `allow` or `deny`.

`connector.json` routes both of those methods and classifies them, and it puts the correlation at
`params.request_id`, so an identifier in an answer is the one the host is resolving rather than a
value a caller supplies. It says nothing about `allow` and `deny`.

That is the whole of what it does about an approval. A table routes and classifies; it names no
destination for a decision, and this package ships no component, so nothing here turns a relayed
request into something a person can read or turns a person's decision into a frame. Both need a
granted decoder and encoder, which this package does not have. An approval is answered where Claude
Code asks for it, in the terminal KalaReach already owns.

The message path is separate, and it is an action with its own effect class. It carries the text a
person wrote into the session. It is not an approval path, and nothing in this package lets text
reach one.

Delivering a message is not steering and not an acknowledgement that anything was processed. Claude
Code queues messages that arrive while a turn is running and delivers them together on the next
turn,
and the write to the transport is the only receipt there is. Claude Code applies whichever answer
reaches it first, the terminal's or another channel's, and drops the other, without telling the
channel which happened.

## What stays in the terminal

Project trust and MCP server consent. Claude Code asks for both in its own terminal and relays
neither, so this package offers no control for them, whatever rights the person holds.

## Runtime gates

The registration is what makes the channel available; it is not what turns it on. Claude Code
registers a channel only when every one of these holds where it runs:

- The session names the server or its plugin at launch. A registration Claude Code can see is not a
  registration it uses.
- The plugin is on the effective allowlist, which is the vendor's own unless an organisation
replaces
  it, or the session was started with the development flag instead.
- The organisation's channel setting permits channels at all.
- The session authenticates in a way that supports channels, which rules out the third-party model
  providers.
- The server declares the channel capability, and the permission capability as well before any
  approval is relayed to it.
- The negotiated protocol revision is one this version of Claude Code registers a channel over.
- The feature itself reaches this installation. It is a research preview, rolling out gradually, and
  a session it has not reached refuses the registration whatever the settings say.

- The skills directory is scanned at all. Managed marketplace policy can suppress that scan, which
  leaves the registration on disk and unloaded.

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
| `skills/kalareach-channels/hooks/hooks.json` | Five hook registrations, each bounded by its own timeout |

The settings key is `enabledPlugins."kalareach-channels@skills-dir"`, set to `true`. The recipe adds
that one key and leaves every other setting where it was.

The hooks are registered on `SessionStart`, `SessionEnd`, `PostToolUse`, `PostToolUseFailure` and
`Notification`. Those five were chosen because none of them refuses what Claude Code was about to do
when a handler exits with a failure code. That narrows the ways a hook can interfere; it does not
remove them, because a handler can answer with a stop decision on most events instead. What this
package asks of the
forwarder is that it answer nothing: write the event to the worker and exit. Each registration also
carries its own timeout, one second for `SessionEnd` and five for the
rest, so a forwarder that cannot reach the worker costs seconds rather than the ten minutes a
command
hook may otherwise take.

The registration installs disabled. Claude Code loads a plugin from a skills directory as soon as it
is there, so the manifest sets its default to off and the settings key is what turns it on. Removal
takes that key out first and then deletes the three files in the reverse of the order it wrote them.
Each file is checked against the digest it was installed with, and one that somebody has since
edited
is reported and left alone rather than deleted, which leaves a registration on disk that the key no
longer enables. A session that is already running keeps what it loaded: Claude Code picks up a
registration change when it reloads its plugins or starts again.

The forwarder runs under Claude Code's own permissions, outside the KalaReach plugin sandbox and
outside Wasmtime, because Claude Code is the process that starts it. The installation grant says
exactly that, and it says what the forwarder can see. None of the three files carries a session
identifier or a secret: what the registration is worth is decided by the forwarder binding it to the
launch KalaReach made and to the worker's private exchange, and the forwarder is the host's, not
this package's.

## What it asks for

Five capabilities. Matching, declarative presentation and broker semantic events are the reading
half. The upstream action capability carries a message you wrote. The bridge installation capability
is declared with the recipe it installs and the grant that says what accepting it means. There is no
approval capability, because nothing here would exercise one.

## Fixtures

`fixtures/conformance.json` states what a person sees in six situations, and the build evaluates
this package's own predicates against each one: a bound session, an actor who may only view,
volatile-native operation, a session with no qualified evidence for acting upstream, an upload in
progress, and a binding disabled by repeated faults.

`fixtures/frames.json` is the other half: five pinned Channels frames, taken from the published
reference's own examples and the field names in the installed executable, each with the method the
table should find, the route and class it should reach, and the identifier the table's path should
extract. The test `every_pinned_frame_is_read_the_way_the_table_says` under `cargo test` reads every
frame through this package's own table, including an answer naming a request nobody issued, whose
identifier has to come back verbatim so the host can refuse it, and an ordinary MCP tool call, which
this table does not list and which is therefore a mutation.

Some of what this package promises is structural rather than conditional, and a fixture cannot state
it. There is no action for project trust or MCP consent, so no right produces a control for either.
There is no action that steers a running turn, so a message delivered to a busy session cannot be
presented as steering.

`docs/qualification-notes.md` in this repository records what every claim above was checked against,
and which claims were not checked against a live install.
