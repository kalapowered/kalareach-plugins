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

Each part the driver runs has a control, and its evidence says whether the control breaks the
part's property (`breaks_property`). For 5a, 6a and 14.03a it does: the part passes only when its
check fails under the control. For 2b and 8a it cannot on a host that binds no connector, since only
a binding advertises or accepts a typed action and only a registration the host issued can change
its state; their controls show instead that the same connection's accepted path works, and the
evidence says why no breaking control ran. Those properties get breaking controls in 2c and 8b.
Each part the driver does not run is recorded as not run, with the reason and what would change it.

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
| `run.own_home` | What the check before the first agent read: the default keychain a session started with the person's own home and no agent names, and the execution context the host gave that session (`worker_profile`, and `bound_to_a_desktop`). The check is the first step of every record, group `own-home`; when it fails, `failures_outside_identifiers` names it |
| `run.packages[]` | The package the parts ran against: `name`, `version`, `manifest_digest` (the SHA-256 of its `plugin.json`) and the `generation` it was installed from. The check requires the version and digest of the package in this tree, so a package released again needs its parts run again |
| `run.applications[]` | The pinned build, and a newer build where the upgrade part uses one: `version`, `url` it came from, `sha256` of its pinned file, `status` and `reason` |
| `tests[].part` | The part the test is, from the table above |
| `tests[].source` | Where the test is, prefixed by its repository: `kalareach:` for the driver's tests, `kalareach-plugins:` for a part recorded from the table above |
| `tests[].needs` | For a part not run, what it needs: a vendor account, a connector binding, a launch registration or draft reads |
| `tests[].evidence` | What a part observed, its control included; under `installed`, the package the host installed, which must be the one in `run.packages`; and under `provenance`, the PATH the session's shell searched, what each launch ran and how it reached the pinned file, and each executable image seen beneath the agent's sessions, with the SHA-256 of the file whose inode the process maps and where it lies (`build`, `newer build`, `runtime`, `run`, `shell` or `system`), from looks every `sample_interval_ms` from the first launch to the end of the part. A process that started and ended between two looks is not there, and one that ended before its image was read is named under `ended_before_read` |
| `tests[].condition` | On a part a vendor gate held: `{gate, observed, fallback}`, where `observed` is `untested` or `unavailable` and `fallback` names the test of the same record that ran the path used instead. The gated part stays `not_run`; the fallback carries its own outcome |
| `attachment_paths[]` | The attachment path each operation declares: every action whose effect carries a prompt or an attachment, then the terminal route; `declared` is `typed_submission`, `verified_composer_insertion`, `terminal_draft_path`, or `null` for nothing declared, and `package_digest` is the manifest digest the declaration was read from |

`run.terminal_profile` is `{profile: "kr-vt/1", term: "ghostty"}`: the windows are read through the
product's own terminal engine, and they name themselves `ghostty`, a terminal that supplies both
enhanced keyboard protocols. Each step's `command` is the driver's command as the harness ran it,
and a test's `command` is its step's. A path in the evidence directory is written `<evidence>/...`,
one in the temporary directory, where each run's own directory is, `<tmp>/...`, one in the Cargo
target directory the host and the driver were built in `<target>/...`, one in the managed shell's
packages `<shells>/...`, one in the tools directory `<tools>/...` and one in the home directory of
whoever ran it `~/...`; the evidence directory itself is named as it is, and so is a system or
runtime executable, since which one ran is the evidence.

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
it, and for a build installed from a wheel, every file digest the wheel's `RECORD` lists against the
installed code.

## Running it

```bash
KR_SHELL_PACKAGES=<managed shell prefix> \
  scripts/e2e-agents.sh --core <core checkout> --tools <agent builds> [--agent kalareach/codex] [--case 2b] [--write]
```

The managed shell is built in the core checkout with `scripts/build-shells.sh --zsh`. No agent signs
in or starts a turn: each runs with its home and its temporary directory inside the run's own
directory on the internal disk, every proxy variable at a loopback port nothing listens on, and a
keychain of the run's own as that home's default, so a secret an agent writes when it starts stays
in the run and the system never asks anyone to create a keychain. The session's shell searches only
the run's link to the build, the run's links to its runtimes and the system's directories. A part
whose session searched or ran anything else, or whose launch did not run the pinned file, did not
test the pinned build, and is recorded as not run, naming what ran.

Before the first agent, the harness starts one session the way a person does, `kr new` with their
own home, no agent and no choice of execution context, and checks that it names their login
keychain as its default. After that check and after each part it reads everything SecurityAgent,
which shows the system's keychain and authorisation dialogs, logged since a minute before its
previous look, and at the end the whole run, and says that nothing was, or stops at once with exit
status 3, as it does when the log cannot be read.
