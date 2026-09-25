# Agent qualification records

A connector table is qualified against one upstream build, and section 12 of the specification
asks for eight cases to be run against that build and recorded per operating system and
architecture. This directory holds those records, one per connector package, pinned build and
platform:

```text
fixtures/agents/<publisher>/<plugin>/<application version>/<os>-<arch>.json
```

`scripts/e2e-agents.sh` produces them, through the qualification driver in the core repository
(`tests/e2e/agents`), and `pipeline/tests/agents.rs` checks on every run that each connector package
has the record for the build its table is pinned to, and that the record is for this release of the
package. A record states outcomes; it does not qualify a case by existing. A case qualifies for an
agent only when every part of it passed.

## The parts

Several cases hold more than one property, and a host can show some of them before others, so a
record names each part on its own. The case numbers are the specification's.

| Part | What it shows |
| --- | --- |
| 1 | Starting the agent from a managed shell, connecting remotely, sending a prompt, adding an image through the attachment path the operation declares, and inspecting the same execution locally |
| 2a | Slash commands, interrupts, queued prompts and steering, each exercised on its own |
| 2b | An agent on its terminal route is advertised no typed capability, and every typed action for it is refused |
| 2c | Under a binding, an action the installed evidence supports is permitted and one it does not support is refused |
| 3 | Local and remote answers raced to one approval: one upstream resolution, an accurate receipt for the loser, no retry after reconnecting |
| 4 | A disconnection after upstream admission and before the reply: identifiers and state reconciled without duplicate work |
| 5a | Killing the control daemon leaves the agent's execution and its local terminal running, and a restarted daemon serves the session again |
| 5b | The gateway's request state survives the daemon's crash |
| 6a | A live process keeps the build image it started with when the installed agent is upgraded, and the newer build starts on the terminal route |
| 6b | Each process stays bound to its own adapter and schema, and a build outside the qualified range is refused the typed routes |
| 7 | The same saved conversation reused from another process is detected as another execution's, not merged |
| 8a | Forged shell titles, transcript paths, session identifiers and hook input leave the host's state unchanged |
| 8b | A forged credential fails at the check where the host's own registration for the same launch passes |
| 14.03a | A path typed at the agent's prompt is terminal input, and reaches the agent's own composer |
| 14.03b | The host never reports a typed path as an attachment the agent accepted |

Each part the driver runs has a control that breaks the part's property on purpose, and the part
passes only when its check fails under the control. Each part it does not run is recorded as not
run, with the reason and what would change it.

## The record

A record is a conformance result in the `kalareach.conformance/1` form: `run`, `steps`,
`identifiers` with a verdict, counts and tests, `failures_outside_identifiers`,
`known_differences`, `problems`, `warnings` and `summary`, with the meanings that form gives them.
The parts of section 12's cases are the tests of `KR-REQ-12.32`, and the two parts of the typed-path
rule are the tests of `KR-REQ-14.03`.

It names `"extension": "kalareach.agent-qualification/1"` and carries these members beyond the
common form:

| Member | Meaning |
| --- | --- |
| `run.host` | `{repository, commit: {id, modified}}`: the core repository and commit the host and the driver were built from, as `run.commit` names the commit of this repository the harness ran from |
| `run.packages[]` | The package under qualification: `name`, `version`, `manifest_digest` (the SHA-256 of its `plugin.json`) and the `generation` it was installed from |
| `run.applications[]` | The pinned build, and a newer build where the upgrade part uses one: `version`, `url` it came from, `sha256` of its pinned file, `status` and `reason` |
| `tests[].part` | The part the test is, from the table above |
| `tests[].source` | Where the test is, prefixed by its repository: `kalareach:` for the driver's tests, `kalareach-plugins:` for a part recorded from the table above |
| `tests[].needs` | For a part not run, what it needs: a vendor account, a connector binding, a launch registration or draft reads |
| `tests[].evidence` | What a part that ran observed, its control included, and under `provenance` the PATH its session's shell searched and every executable image a process beneath its sessions ran, with the file's SHA-256 and where it lies: `build`, `newer build`, `runtime`, `run`, `shell` or `system` |
| `tests[].condition` | On a part a vendor gate held: `{gate, observed, fallback}`, where `observed` is `untested` or `unavailable` and `fallback` names the test of the same record that ran the path used instead. The gated part stays `not_run`; the fallback carries its own outcome |
| `attachment_paths[]` | The attachment path each operation declares: every action whose effect carries a prompt or an attachment, then the terminal route; `declared` is `typed_submission`, `verified_composer_insertion`, `terminal_draft_path`, or `null` for nothing declared, and `package_digest` is the manifest digest the declaration was read from |

A path the run observed under the tools directory is written `<tools>/...`, and one under the home
directory of whoever ran it `~/...`, so a record names no machine's layout beyond its evidence
directory.

## The build list

`builds.json` pins each connector package's build for this platform: where it comes from, where it
is installed under the tools directory (`prefix`), the file whose SHA-256 names it (`pinned`), the
command a person types, the runtimes it needs (each linked by its own name into a directory of the
run's own, which the session searches after the build's `bin` and before the system's), the
variables that switch its updater and telemetry off, files its vendor's own first-run steps leave
in its home, the text its first screen shows, one harmless input with the text it shows, how it
reaches its composer where it has one without an account, the server its terminal route starts
first where it has one, where its vendor keeps a conversation, and the newer build the upgrade part
moves to where one is named. The harness checks each build's digest before it runs anything with
it.

## Running it

```bash
KR_SHELL_PACKAGES=<managed shell prefix> \
  scripts/e2e-agents.sh --core <core checkout> --tools <agent builds> [--agent kalareach/codex] [--case 2b] [--write]
```

The managed shell is built in the core checkout with `scripts/build-shells.sh --zsh`. No agent signs
in or starts a turn: each runs with its home inside the run's own directory on the internal disk,
every proxy variable at a loopback port nothing listens on, and a keychain of the run's own as that
home's default, so a secret an agent writes when it starts stays in the run and the system never
asks anyone to create a keychain. The session's shell searches only the run's link to the build,
the run's links to its runtimes and the system's directories. A part whose session searched or ran
anything else did not test the pinned build, and is recorded as not run, naming what ran.

Before the first agent, the harness starts one session the way a person does, with their own home
and no agent, and checks that it names their login keychain as its default. After that check and
after each part it reads what SecurityAgent, which shows the system's keychain and authorisation
dialogs, logged meanwhile, and says that nothing was, or stops at once with exit status 3.
