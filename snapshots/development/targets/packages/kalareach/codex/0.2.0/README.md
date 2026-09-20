# kalareach/codex

Recognises Codex CLI, reads its App Server through a declarative table, and stops the turn it is
running.

## What it does

KalaReach matches the `codex` executable, labels the session, and runs Codex through its ordinary
terminal path. That terminal stays exactly as Codex draws it. What this package adds on top is a
reading of what Codex is doing, and one control over it.

`connector.json` is the declarative native-proxy table. It states the framing (one JSON document per
line), where a request carries its identifier, where a message names its method, how a response is
matched to its request, which methods travel which way, and what each method does. Core code reads
that table directly, so no component of this package's sits between Codex and the host on the
forwarding path.

The table is pinned to the App Server protocol of Codex CLI 0.155.1 and qualified against that
version alone. A protocol revision is exactly the kind of change that moves a method between
classes, so the table carries forward to nothing.

## What it classifies

Forty-nine methods, each with the evidence it was classified against. Reads are observations.
Starting, steering, interrupting and compacting a thread are mutations, and so is every reverse
request whose answer changes what the agent does. The account methods are credential methods: the
broker keeps their answers and never passes them to a component.

Nine methods are declared unsupported rather than proxied. `thread/shellCommand` and `process/spawn`
run outside Codex's sandbox; `command/exec` runs a command with no thread or turn to bind it to; the
`config/*` writers rewrite the user's `config.toml`; the `fs/*` methods reach absolute paths through
Codex instead of through KalaReach's own file grant; and `account/logout` discards the credentials
somebody signed in with.

A method this table does not list is treated as a mutation. That is the safe answer to a method a
publisher forgot, and nothing in the file changes it.

## What the table covers, and what it does not

It covers the connection between KalaReach and the App Server: JSON-RPC 2.0 over the bound process's
standard streams, one document per line, identifiers at `id`, methods at `method`, responses matched
by repeating the `id`. On that connection the App Server meets the declarative proxy contract, which
is why this package installs no native bridge and edits none of Codex's settings.

It does not cover the native `--remote` connection. That client speaks WebSocket, over TCP or over a
Unix socket with an HTTP upgrade, which the table's framing vocabulary cannot describe. The leg
between the native terminal and the gateway is the gateway's own listener, and this package makes no
claim about it. `docs/qualification-notes.md` in this repository records that gap alongside the rest
of the qualification.

The message size the table declares is the bound this host enforces while reading frames, not a
limit Codex states. A response larger than it fails the connection rather than being read in part.

## What it does not carry

No component, and therefore no approval decoder. A pending Codex approval stays with the gateway and
the terminal Codex drew it in; this package offers no button that would answer it.

No prompt or steer action either. A control supplies parameters, and the declarative binding has no
way to say "the broker fills the bound thread here", so an action that sent `turn/start` or
`turn/steer` would have to let a control name the thread. A control that can name a thread can name
the wrong one.

Resuming a saved thread reads stored history. It does not establish that this process owns the live
execution, and nothing in this package treats it as though it did.

## Fixtures

`fixtures/conformance.json` states what a person sees in nine situations, and the build evaluates
this package's own predicates against each one. The cases are named for what they assert, which is
which controls a person sees and which of those they can use. They are display conformance, not
protocol conformance: the fixture runner evaluates predicates and never reads a Codex frame.

The situations the Codex integration has to be exercised against map onto them like this, with the
protocol half of each owned by the gateway rather than by this package:

| Situation | The case that fixes this package's half |
| --- | --- |
| Thread subscriptions | An idle bound thread has no turn to stop |
| `turn/steer` and its `expectedTurnId` | A running turn can be stopped |
| An uncertain `turn/start` result | A turn whose start was never confirmed leaves the binding idle |
| Competing approvals | A turn waiting on a person can still be stopped, and no approval control appears |
| `serverRequest/resolved` | An actor who can only view sees no turn control |
| Reconnect | Volatile-native operation leaves no usable turn control |

A predicate edit that changes any of those answers fails the build.
