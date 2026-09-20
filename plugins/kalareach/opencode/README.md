# kalareach/opencode

Recognises OpenCode, reads its shared server's event stream through a declarative table, and stops
the turn it is running.

## What it does

KalaReach matches the `opencode` executable, labels the session, and runs OpenCode through its
ordinary terminal path. That terminal stays exactly as OpenCode draws it, bound to the same backend
the session reads. What this package adds is a reading of what that backend is doing, and one
control over it.

`connector.json` is the declarative native-proxy table. It states the framing, where a message
carries its identifier, where it names its method, how a response is matched to its request, which
methods travel which way, and what each one does. Core code reads that table directly, so nothing of
this package's sits on the forwarding path.

## Which API family, and why that matters

OpenCode's server answers two distinct API families from the same build, and they are not
interchangeable. The shared per-user family serves `/api/...`, and its durable event stream at
`GET /api/event` carries each event as an envelope with `type`, `id` and `data`. The earlier family
serves the unprefixed routes, and its stream at `GET /event` publishes the same event names with the
payload under `properties` instead.

This package is the shared-server family's profile. It is pinned to OpenCode 1.18.31 and qualified
against the OpenAPI document that build serves, so the adapter is chosen by the installed version
and the schema rather than by the name of an executable. A table qualified here finds nothing at the
other family's payload paths, and `fixtures/frames.json` pins an envelope from that family to prove
it. When the installed family is not this one, the capability is incompatible, every control this
package draws disappears, and the terminal is what is left. The earlier family has its own profile,
with its own record, match rules and version line.

## What it classifies

Eighty-eight event names, each with the evidence it was classified against. An event that reports
what the session did is an observation: turn steps, text and reasoning chunks, tool calls and their
results, changed files, the sessions that are idle, and the permissions and questions the session is
waiting on. None of them carries a credential.

Four are not reports. `tui.session.select` moves the attached terminal to another conversation,
which changes the execution a binding is observing, and `tui.toast.show` writes on that terminal's
screen; both are mutations. `tui.prompt.append` writes into the native composer and
`tui.command.execute` runs a named terminal command, including `prompt.submit`. This package
qualifies no composer insertion and no terminal control surface, so the table declares both
unsupported rather than proxying them.

An event this table does not list is treated as a mutation. That is the safe answer to a name a
publisher forgot, and nothing in the file changes it.

## What the table covers, and what it does not

It covers the event stream: the leg on which the backend tells every attached client what happened.
That leg has a framing, a method name at `type`, an identifier at `id` and a correlation that finds
the same identifier, which is what the declarative proxy contract asks for.

It does not cover the request leg. Asking that server to do something is an HTTP request, whose
method is a verb and a path rather than a member of a document, and a connector table names methods
by a path into a decoded message. `docs/qualification-notes.md` in this repository records that gap
alongside the rest of the qualification.

Interrupting a turn therefore goes through the broker's own cancellation against the bound
execution, not through a method this table routes. The message size the table declares is the bound
this host enforces while reading frames, not a limit OpenCode states.

## What it does not carry

No component, and so no approval decoder. A pending OpenCode permission stays with the gateway and
the terminal that asked for it; this package offers no button that would answer one.

No prompt action, and no attachment contribution. The ordinary route is the terminal, where the
composer's own syntax is what inserts a file, and this package qualifies no composer syntax and
claims no automatic insertion.

Reading a stored session is not evidence that this process owns the live execution, and nothing here
treats it as though it were. Nothing in this package adopts a shared backend holding somebody else's
sessions: the binding is to the execution the launch produced.

## Fixtures

`fixtures/conformance.json` states what a person sees in eight situations, and the build evaluates
this package's own predicates against each one. They are display conformance and only that: the
runner reads no OpenCode frame. One case is the installation of the other API family, where both
controls disappear and the terminal path is what remains.

`fixtures/frames.json` is the other half: nine event frames written from the event schemas in the
OpenAPI document the installed build serves, completed against each variant's required members. Each
carries the method the table should find, the route and class it should reach, the identifier the
table's path should extract with its JSON type, and the members its own situation is about. The test
`every_pinned_frame_is_read_the_way_the_table_says` under `cargo test` reads every one of them
through this package's own table, and `a_second_family_frame_is_refused_rather_than_read` checks the
two that belong to the other family. The frames also pin the evidence they were written against, by
digest.

Each is a frame, not a sequence. What a host does across two pending permissions, or across a
reconnection to the shared server, is the gateway's and is not settled here.
