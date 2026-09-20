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
that table directly. No component runs on the forwarding path, so nothing this package ships can
stall Codex's own traffic.

The table is pinned to the App Server protocol of Codex CLI 0.155.1 and qualified for the 0.155
series. A protocol revision is exactly the kind of change that moves a method between classes, so
the table does not carry forward to a version nobody tested.

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

## What it asks for

Four capabilities. Matching and declarative presentation are inside the ceiling a newly enrolled
repository already permits. Broker semantic events carry the turn and item stream the session is
already authorised to see. The upstream action capability exists for one control: Stop the turn.

## What it does not carry

No native bridge. The App Server meets the declarative proxy contract, so there is nothing for
bridge code to close, and this package installs no file into Codex and edits none of its settings.

No component, and therefore no approval decoder. A pending Codex approval stays with the gateway and
the terminal Codex drew it in; this package offers no button that would answer it. Nor does it
declare a prompt action: prompts and steering reach the bound thread through KalaReach's own agent
path, which carries the thread and the turn the person saw rather than letting a control name them.

## Fixtures

`fixtures/conformance.json` states what a person sees in eight situations, and the build evaluates
this package's own predicates against each one. An idle subscribed thread, a running turn, a turn
whose start was never acknowledged, competing approvals, a resolved server request, a reconnect that
left the host in volatile-native operation, an upload in progress, and a binding disabled by
repeated faults. A predicate edit that changes any of those answers fails the build.
