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
has the record for the build its table is pinned to, and that every record, pinned or kept after its
build was withdrawn, is for this release of its package and was run against the build its path
names. A record states outcomes; it does not qualify a case by existing. A case qualifies for an
agent only when every part of it passed.

## The parts

Several cases hold more than one property, and a host can show some of them before others, so a
record names each part on its own. The case numbers are the specification's.

| Part | What it shows |
| --- | --- |
| 1 | Starting the agent from a managed shell, connecting remotely, sending a prompt, adding an image through the attachment path the operation declares, and inspecting the same execution locally |
| 2a | Slash commands, interrupts, queued prompts and steering, each exercised on its own |
| 2b | An agent started by hand on its terminal route is detected as a manual launch and offered only observation and the terminal: no typed capability is advertised, and every typed action for it is refused |
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
part's property (`breaks_property`). For 2c, 3, 4, 5a, 6a, 7 and 14.03a it does: the part passes
only when its check fails under the control. For 2a, 2b and 8a it cannot: only the agent could
refuse its own terminal keys, only a typed route could advertise or accept a typed action, and only
a registration the host issued could change its state. Their controls show instead that the same
connection's accepted path works, and the evidence says why no breaking control ran.

What the host shows about an agent started by hand is checked against what section 12 requires of
a launch the command integration did not make, not against what it happens to announce: the
launched execution is detected as one instance of the installed package, integrated as a native
terminal with every bridge refused, with a binding a device can read; it is offered only
observation and the terminal, with no typed capability usable, every typed mutation refused, no
pending resource and no command backend; and steps that keep the process, its conversation and its
upstream owner leave the instance and its binding revision as they were. Each of those checks is
also shown observations made wrong on purpose, which it must reject; the evidence names those runs
`checker_controls`. A launch the host does not detect fails parts 1, 2b, 2c and 7, whose properties
need the detected instance, and the rest of their checks are kept as evidence.

Parts 1, 2a, 3, 4 and 7 need the person's vendor login, and run only for a build whose list entry
names a login the person approved; each part not run says why and whose decision would change it.
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
| `run.applications[]` | The pinned build every part ran against, first, then the newer build where the upgrade part uses one: `id`, `version`, `url` it came from, `sha256` of its pinned file, `status` and `reason`. The first is the only build the record says anything about |
| `tests[].part` | The part the test is, from the table above |
| `tests[].source` | Where the test is, prefixed by its repository: `kalareach:` for the driver's tests, `kalareach-plugins:` for a part recorded from the table above |
| `tests[].needs` | For a part not run, what it needs: a vendor account, a connector binding, a launch registration or draft reads |
| `tests[].evidence` | What a part observed, its control included; under `detection`, what the host announced about the launch; under `checker_controls`, each check shown a wrong observation and whether it rejected it; for a part with a login, under `account`, the login's kind, where it is kept, whose home the agent ran with and the turns the part started, never an identifier or a value, and, where the agent ran with the person's home, under `person_home`, what the part created there and removed, and every other change it saw; under `installed`, the package the host installed, which must be the one in `run.packages`; and under `provenance`, the PATH the session's shell searched, what each launch ran and how it reached the pinned file, and each executable image seen beneath the agent's sessions, with the SHA-256 of the file whose inode the process maps and where it lies (`build`, `newer build`, `runtime`, `run`, `shell` or `system`), from looks every `sample_interval_ms` from just before each launch until the part's sessions ended, and one more when the part's own steps ended; each look reads every process's mapped image, so an image that changes under the same process is seen, and hashes it through one handle on the mapped file. `pids` lists every process number seen beneath the sessions, and `system_programs_of_another_user` each of the system's own set-user-ID programs seen there (such as `/bin/ps`, which an agent runs), which the kernel will not describe to the person's user, by number and the protected file it runs. A process that started and ended between two looks is not there, and one that ended before its image was read is named under `ended_before_read`. `launches` says how each launch reached the pinned file |
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
command a person types, how a launch reaches the pinned file (`launch`: `native`, its own process
maps it; `child`, a runtime starts a process that maps it; `script`, a runtime's first argument is
it; `wheel`, a runtime loads the installation the harness compared with the wheel), the runtimes it
needs (each linked by its own name into a directory of the run's own, which the session searches
after the build's `bin` and before the system's), the variables that switch its updater and
telemetry off, files its vendor's own first-run steps leave in its home, the text its first screen
shows, one harmless input with the text it shows, how it reaches its composer where it has one
without an account, the server its terminal route starts first where it has one, where its vendor
keeps a conversation, and the newer build the upgrade part moves to. An entry whose agent the
person approved a login for names it under `account`: the login's kind and where it is kept, whose
home the agent runs with (`run`, or `person` for a login kept in the agent's own files there),
whether the login is a login keychain item (a run home then borrows the person's login keychain,
and the agent's sessions run in the person's desktop, where a keychain can be read, rather than
the host's headless context the other parts use), the one variable a login may be, the arguments
these parts type (a model, a sandbox), and the switches the agent and every probe of it are given
(`switches`, with `{work}` for the working directory as the system resolves it and `{run}` for the
run's own directory; and
`server_switches`, the file and table in which the person's configuration names its servers and
the words that switch one off by name, for each server named there). A probe is a command the
agent answers without calling a model: `status` says whether the login holds, and each of
`isolated` that the agent loads none of the person's own servers, hooks, plugins or skills. A
probe names texts its answer holds, spaces aside (`shows`), lines it holds whole (`lines`), and
texts it must not hold, compared without regard to case (`lacks`, with `{servers}` for each server
the person's configuration names). Where the answer is JSON, `shows_within` names the block the
texts it shows must be in, and `accepted` the one block the probe accepts although it holds some
of what it must lack: exactly one string may begin with `accepted.starts`, it must carry the
person's file `accepted.file` whole, such as the instructions file the agent always reads, and only
that file's text goes unsearched. A probe whose arguments hold `{stub}` points the agent at a stub
on this machine in place of its vendor's model service: the stub keeps only the kind and name of
each tool the agent's first request offers, with the tools a tool's description offers in turn by
headings of their own (a tool that runs code lists what the code may call) and whether it says it
leaves some out, refuses the request, and those names are the answer and are recorded under
`tools_offered`, with a control in which the same check must refuse one more tool named for the
first text it must lack; no model is called and no login is sent. The entry also names the files
that would load the person's own settings, hooks or servers into the agent or that it would remove
(`absent`, with `{home}` for the home it runs with, none of which may exist before it starts), the
texts the agent shows when it is signed out, a
configuration directory of the run's own and the variable that names it, variables the session is
given as they are, variables that would steer the agent's model or account or move its home and
that its session must not export (`cleared`, each a name, a prefix that ends in `*` or a suffix that begins with one, never one of the product's own `KR_` names, but for the
names the entry sets itself, the one variable its login is among them: every entry that names an account clears the same provider variables, the keys, tokens, secrets and base addresses of every provider and the cloud accounts an agent can read its model from, and adds those its own agent documents; the environment a part's session is created with has them taken out whatever put them there, a real session made from an environment that holds them is checked before any part with a login, and the session's shell writes the names it exports, never a value, before
each prompt and again just before it runs a command line; a part with a login whose shell exports
one of them, or whose names cannot be read whole, does not go on: the agent is not started, or is
ended before anything is typed to it, and the part is recorded as not run, or as failed with the
code beside those of what stopped the agent where something did, and a part that starts it records the names under `account.variables_absent`), the name of the
login's keychain item, whose modification time says whether a
part rewrote it, the files of the person's home no part may change (`guarded`), those the
person's own programs also write (`shared`, which stop the agent only when a change names the
run's directory or the part's mark), those whose change is only recorded (`recorded`, such as a
login the agent refreshes), the files the agent and the person's other programs only append lines
to (`append_only`: a line that was there before the part changing or going stops the agent, and
the lines appended since are recorded, the part's own, those holding its mark or the run's
directory or naming a conversation it created, each with the conversation it names (`id`, or
`session_id`) and its time (`updated_at`, or `ts`), each kept only in the shape of an identifier
or a time, and which of those it holds, and the others only counted, and all are left as they are), and the agent's line files (`line_files`, watched as `append_only`
files are, only files whose every writer takes an exclusive lock on the file for each line: under
that lock the part's own appended lines, holding its mark or the run's directory, are removed);
the budget its turns are charged to and its limit;
the agent's own directories in the person's home (`directories`, with `{date}` for the local date
as `YYYY/MM/DD`, the day the part starts and the next, and a trailing `/*` for a directory read
for the files directly in it) and those of them whose files belong to one conversation or run each
(`removable`: a file the part created there holding its mark or the run's directory is removed,
and one it created anywhere else, such as a database the person's own sessions share, is reported
and never touched), where it keeps its conversations (`conversations`, with `{date}` too)
and how their prompt, reply, tool-decision, turn-start and queued-prompt lines are marked (with
`decision_calls` marking the calls that ask for approval, where only some do: then only the
answers tied to such a call by its `call_id` count); the mark its screen shows at each reply,
where it has one; and the screens and keys its terminal controls use, among them the key that
queues a prompt behind a running turn where it is not the submit key, the key that steers a prompt
already queued (`steer_key`, sent once `steer_ready` shows, the prompt recorded by `steer_line`),
and its permission dialogs:
the command dialog, answered only when one line of it is the part's command alone (after
`approval.command_line`, where the dialog shows a prompt mark before a command, with no other
command line, and with only blank lines between it and the dialog's first choice,
`approval.options_start`), `deny` for a command the part refuses, and the texts of every other
dialog (`approval.others`), each refused with `approval.refuse`, which leaves the agent no rule. Where the folder's absolute
path does not fit a line of the dialog, `approval.relative_log` names the command's log relative to
the folder, and the part answers only when the agent's own record of the request
(`approval.request`) is the pending request of the tool that runs commands and names that command and the
run's folder. A
dialog shown while a part waits for anything is refused before anything else the screen shows is
taken, in the waits for the agent's screen and while the device is away, from the local terminal
then, and every refusal is recorded; a dialog is read again just before it is refused, so a key is
never sent after it closed. Part 3's command names its log by its absolute path, so where the
agent runs it does not change what it writes. Where only some calls ask for approval, the records
are read by kind (`call_lines` marks each kind of call record): the answers counted are the calls
that asked and have an answer tied by `call_id`, and a call that holds more than one request, a
call or answer without an identifier, or an answer tied to no call leaves the count unknown and
fails the part. Since one call of a tool that runs code can ask more than once whatever its text
holds, part 3 also requires that the part refused nothing from its prompt to the end of its
control: any request besides the one it answers reaches a dialog, which the part refuses and
records, and that fails the part. `resume_forks` says the resume arguments start a new conversation from
the saved one because the agent lets no second process write a conversation another writes; the
second-process part then says it did not show two processes on one conversation's identifier. The
composer waits for a prompt while the screen shows `composer` and not `busy`. While a part's own
steps run, the probes among them, a thread of its own reads the files no part may change, those
the person's own programs also write, and those only appended to (for their earlier lines), every
quarter second; the part's steps also read them
before each probe, just before the agent's command is typed, before each prompt and approval, and
in their waits. After a change is seen the part sends nothing more, and the stop ends every
process it can reach, waiting on nothing of the provenance sampler: the probe that runs (each runs
in a process group of its own, listed under the same lock the stop takes, so a probe is either
ended by the stop or refused by it); then it halts the sampler, which from then on kills each
process it would take, and stops (SIGSTOP) the run's recorded processes, the sessions' shells, the
processes each launch found and those the sampler published the moment it took them, and, pass by
pass, every process beneath one it stopped, each the moment it is found and confirmed that
process's child, until a pass finds nothing new; then it kills them all. The part's outcome
records how many it saw stopped, those it signalled but did not see stopped, why any could not be
looked at or stopped, whether its walk was complete, and which still ran two seconds later. A
process that left the tree before the stop reached it, its parent having ended, and that the
sampler had not seen, is outside what the stop can reach. The part then stops on the change and
its agent stops. A part whose login cannot be
established that way, whose agent does not reach its composer with it or shows it is signed out,
or that ends without passing while its latest request to the model has no answer only the model
could have given (a sum, the part's code in upper case, a tool request), stops that agent's
remaining parts in the run. An
entry without one names under `no_account` why its parts with a login do not run. The harness
checks each build's digest before it runs anything with it, and for a build installed from a wheel,
every file digest the wheel's `RECORD` lists against the installed code, and the launcher a person
types against the `RECORD` the wheel's own distribution installed, which must be the only one naming
it.

## Agents that run on a provider's key

OpenCode and Gemini CLI have no login file of their own to borrow here, only a provider's key in the person's shell: `OPENROUTER_API_KEY` for OpenCode and `GEMINI_API_KEY` for Gemini CLI. Their entries run with the run's own home and name that variable (`account.variable`). The harness hands its value to the driver on a pipe; the agent's session and the isolation probes of its entry get it, and nothing else does. The provider keys, tokens, secrets and base addresses that the agents' own catalogues name, and the families around them, are cleared from the session (`cleared`: a list that is a name, a prefix or a suffix, the same for every entry and added to by an agent's own names). The run's directory and the harness's evidence are searched for the values of the keys the harness's own shell holds whose names end in one of the secret shapes (a name of another shape is cleared but its value is not searched for). A key that reaches either stops the agent.

An entry turns the agent's updater, sharing, telemetry and project and external configuration off through variables (OpenCode: `OPENCODE_CONFIG_CONTENT` and the `OPENCODE_DISABLE_*` variables) or through a settings file in a configuration directory of the run's own (Gemini CLI: `GEMINI_CLI_HOME`). OpenCode runs and edits without asking unless it is told to ask, so its configuration makes both ask and the approval case has a dialog to answer.

OpenCode keeps its conversations in a SQLite database, so its entry names a `mirror`: the database, relative to the home the agent runs with, and a `SELECT` that returns a conversation's identifier and one line of JSON for each fact of it, in the order each fact became true. A user's prompt stands where the agent began the turn for it, a prompt entered during a turn has a queue record where it was entered, and a reply stands where it was last written. While a part runs the driver runs the query read-only, writes one file of JSON lines for each conversation and reads those files; the record says the conversation was read through a mirror. Where an agent needs its interrupt key pressed twice, `interrupt_presses` says so; where a conversation's identifier is a member of the first line of its file and not its name, `conversation_id_member` names the member. Its database keeps no permission replies, so the mirror's approval and answer lines stand for each tool call and the end state it reached: the part's count of answers is a count of tool calls that ended, and its check that the command ran once is the evidence of one execution. A row of a dialog is read without the characters its agent draws its frame with, and an entry with `boxed` also requires the command to stand alone in its box.

## The sandbox profile and the proxy

Kimi Code keeps its data in `~/.kimi-code`, a directory the person's own sessions share, so its login parts run confined. The agent's `account.confinement` entry names the committed profile (`fixtures/agents/sandbox/kimi-code.sb`), the data directory and the variable that points the agent at it (the agent's home stays the run's own), and the hosts the run's proxy may reach. It also names the variables that carry the proxy's address, the configuration table that holds the login, the file that lists the person's servers and the project file that switches each one off, the tools the agent runs without asking, the process names the agent goes by (`process_names`), and the data directory paths whose rewrite is only reported (`reported`).

The kernel enforces the profile on the agent's whole process tree and on every program a tool starts. `sandbox-exec` applies it and runs the agent in the same process, so the agent is still the shell's own child. The profile takes its parameters as `-D name=value`: the run's home, folder, temporary directory, empty skills directory and links to the build, the pinned build, the managed shell's packages, the data directory, the directory of sessions, the prompt history and the file-history record for the run's folder, the credentials file in use, and the proxy's loopback port. Nothing else in the file varies.

The agent may read and write the run's own places, and read the pinned build, the managed shell and the system's directories. In the data directory it may write only what it keeps there while it runs: the run's own sessions and the two indexes beside them, its history and file-history record for the folder, its log, cache and search index, its workspace list and index, its small state files (`device_id`, `region` and `migrations-effort.json`), the files in `oauth/`, where its login lock files are, and the login in use. It may not write another session, history or record, its configuration, servers, instructions, skills, binaries or trust records, its telemetry or updates, or the credentials of a login it is not using. It can write to the terminal it was started in and no others. It runs no program from the data directory or from any place the run can write, and it maps no library as code from the data directory, the run's folder, temporary directory or skills directory; the build's own native modules, which it extracts into the run's home, are mapped. It can only signal its own processes, and cannot read other processes' environment or command line. It uses no POSIX shared memory, and opens no device but the few every program uses. It cannot register any service and can only lookup a small number of system services that are needed to start. Finally, network access is disabled, except for a single port on the loopback interface.

A profile cannot allow an address, so that port belongs to the run's proxy. The proxy answers `CONNECT` only for `<host>:443` with a host named in the entry. It resolves the name once, refuses the tunnel unless every address is a public unicast address, and connects to one of them. It refuses and counts everything else. The agent's proxy variables point to it, and its web search and fetch base URLs point to a closed loopback port, so the agent's own web tools fail. The proxy does not read what a tunnel carries. It cannot see the TLS server name, and the hosts share edge addresses with others. A confined part's record states the profile's SHA-256, the proxy's hosts, its count of tunnels by host and of refusals, and the tools the agent runs unasked, as conditions of the result. The person's own use of the agent has no such wrapper. The part's private log lists each tunnel's destination address, time and byte counts.

The part parses the person's agent configuration as TOML and accepts only the top-level keys, the permission table's keys and the rule keys whose use by the pinned build it has read. It stops before the agent starts if the configuration has any other key, any permission mode, any rule that allows a built-in tool, or any source of skills, agents, hooks or plugins.

While the part runs, it looks at the login's files about every quarter second and keeps every string of 16 characters or more that they hold at a look. A token that the agent writes and replaces between two looks is not seen. After the part, before it removes anything the run left in the data directory, it searches for those strings together with the ones read before and after. The strings are those of every credentials file and of the configuration's keys. It searches the run's directory, the part's own evidence directory as written so far, and what the agent wrote into the data directory: the run's sessions, the history and file-history directories, the whole log directory, the agent's small state files, its indexes and every path in `reported`. A hit stops the part. So does a failed or incomplete search, including a look at the login's files that failed while the part ran. The part's result is held against the same strings before it is written. A result that holds one becomes a failure, whatever its outcome was. Each string and key that held one is replaced, the rest of what the part observed stays, and the record says why the agent stops. If a replacement cannot remove one, as with the digits of a number, the result keeps only the part, its process numbers and the stop, and where that still holds one, the stop alone. Furthermore, if any string from the login's files contains a quote, brace, bracket, backslash, or control character, it cannot be removed from the result, so the part is not run. A look that fails, or that finds one, stops it at once; the strings that the look found and that can be searched for are still searched for. The harness then searches all the run's evidence for the credentials files' strings as read before the part and after it, and does not start a part if it cannot read them.

The part also looks in the run's sessions for a subagent and for any prompt that a person did not type, such as a goal's continuation. Their model requests are not turns the part charged, so either one stops the part at once.

The agent can also ask its provider for a conversation's title. That is a request to its model that no submission of the part made. After the part, before the run's sessions are removed, the part counts the lines of the run's own conversation logs that name a title request that failed, and the run's conversations whose own record says a request titled them. A conversation's log writes that line at debug level, which a run does not enable, so only a request that succeeded shows. The pinned terminal makes no such request. A part that sees one charges it to the ledger as a turn, counts it in the record as `titles`, and stops.

A reported path lies inside the data directory and holds no credential. A rewrite there is reported by size, time and digest, and only where the file was read whole before and after the part. Any other rewrite, and any removal outside the reported paths, stops the part. The log's numbered rotations are reported paths.

The part also stops when a process beneath the agent has a command line that names one of the person's servers. A process of the agent's name (`process_names`: the program's own name and the title the agent gives itself) that runs when the part starts stops it before it starts. One that is not beneath the run and appears later does not stop the part. It may have written the data directory, so the part puts nothing back over the workspace list when one runs, or when one was seen. A process counts as beneath the run only through a process the run recorded that still runs, by its start time. The agent's own record of the run is also counted against the turns the part charged. A prompt counts whether a person typed it, it steered a running turn, or it came from another origin, and a surplus stops the part and is charged to the ledger.

The key that submits a prompt is sent only after a fresh screen shows none of the agent's dialogs, because it would pick the dialog's first choice. The same check runs before the prompt is typed, since a typed digit also picks a choice. Part 3's control prompt, entered at the local terminal, is not checked this way.

After the checks, a launch that does not resume a conversation starts a fresh one with `/new`, which makes no request to the model, so the checks' shell lines are not part of the conversation the part's turns are in. A launch that resumes a conversation runs none of the checks, because its shell lines would enter the saved conversation. The checks of the part's first launch ran the same profile and cover it. The number of times the agent is launched to resume a conversation will be recorded in the result as `resumed_launches`.

Before the first turn, each start that does not resume a conversation shows with the agent's own `!` line, which runs a shell command with no model call, that a file outside the profile cannot be read where one inside can, that a file outside cannot be written, that a direct connection to a provider address of either family is refused, that the proxy refuses a host it was not given and tunnels to one it was, that the person's own copy of the agent cannot run, and that no connection in its tree goes anywhere but the proxy. A digit and Ctrl-U sent through the device's keyboard reach the composer. The agent's server list shows each of the person's servers disabled, with no tool available.

A confined part's record also lists what its checks do not cover, in the build list's own words (`residuals`). Detection is by whole string, and the agent's own record of the conversation is the backstop. A request can go out before the part stops it. The proxy counts tunnels, not requests. The person's older data in the history and log directories is searched whole. The agent's `oauth/` directory is not searched, because it holds the agent's own credentials. A failed title request leaves no line at the level a run uses, so it is not seen. A launch that resumes a conversation skips the checks.

## What the index takes from them

The catalogue pipeline names a pinned build in its release's index entry only when the record for
that release, build and platform is in good form, is a whole run of this release whose parts ran
against this build (the first of its `run.applications`, installed, with the pinned SHA-256), shows
no part failed, and shows every part of the eight cases (1 to 8b above) passed. A part not run
is not a pass. The pinned file must be what a process runs: a build whose `launch` is `script` or
`wheel` is never named, since a script runs as its interpreter and a wheel is an archive. The parts
of the typed-path rule, 14.03a and 14.03b, say nothing about the build and are not required, but a
failed one keeps the build out. `docs/pipeline.md` describes the rest.

## Running it

```bash
KR_SHELL_PACKAGES=<managed shell prefix> \
  scripts/e2e-agents.sh --core <core checkout> --tools <agent builds> [--turns <ledger>] \
    [--agent kalareach/codex] [--case 2b] [--write]
```

The managed shell is built in the core checkout with `scripts/build-shells.sh --zsh`. A part that
needs no login signs in nowhere and starts no turn: the agent runs with its home and its temporary
directory inside the run's own directory on the internal disk, every proxy variable at a loopback
port nothing listens on, and a keychain of the run's own as that home's default, so a secret an
agent writes when it starts stays in the run and the system never asks anyone to create a keychain.

A part with a login runs the agent as its entry's `account` says, with no proxy variable. The
driver always runs with a cleared environment that names only the home, the user, the temporary
directory, the system's PATH and the harness's own inputs, so nothing else the harness's shell holds
reaches the host or any program the driver starts. Where the agent runs with the run's own home,
that home either searches the person's login keychain, which the run borrows and never creates or
deletes, or the agent's session is given the one variable the login is: the harness hands its value
to the driver on a pipe, and only the agent's session environment gets it. After the part, however
it ended and once everything it started has ended, the run's directory is searched for the value,
every file that held it is recorded by path in `key-scan.jsonl` beside the part's log, never the
value, a search that could not read every file fails the part, and the directory is removed and
checked gone; the harness then searches every file of its evidence directory, the part's log
included, and stops on the value or on a file it could not read. Where the agent runs with the
person's own home, its directories are listed before and after each part, the files the part
created for its conversations are removed, the part's own lines appended to the agent's line files
are removed under their writers' lock, no file or directory that was there before is removed or
edited, no line of an `append_only` file is removed or edited, and each
change that stays is in the part's evidence, with those of the changed files left that hold the
part's mark or the run's directory, which nothing touches; its configuration directory of the
run's own, where it has one, is removed after the part and checked gone, and the guarded and shared
files are compared before and after, by SHA-256 in `guarded-files.jsonl` beside the part's log and
by whether each changed in the record, and the files only appended to by whether their earlier
lines are intact and what was appended. A published record of a part run with the person's login
keeps only what the driver fills from the build list, the run and the part's own marks, fixed
result and failure codes, and counts. `scripts/record-redaction.jq`, which the harness applies to
every record it writes, projects the part's evidence through a schema: a key the schema does not
name is dropped, and a text it names is kept only where it is one of the schema's own fixed texts,
a value of the build list's entry, or a shape the driver makes (an identifier, one of the part's
marks, a path in the run's own directory, a time); any other text becomes `<not published>`. What
the host answered a call with is the call and the code it was refused with, never its
diagnostic; a screen's rows are whether the part's mark showed; the tools a probe found offered
are a count; a process is its number, identity and digest, without its command line; and the
person's servers are `<server 1>` to `<server n>`, by their place in the list of servers switched
off and only where the driver put a server's name (that list, the switches that turn them off, and
a control that added one, which is only said to have: `control.server`), so a server that has the
name of a word of the build list or of a fixed text changes none of them. A failure is a code and what varies in it
(`evidence.failure_codes`), and the part's `reason` is the fixed words of each code, or "a failure
the part's log states": the text that described a failure can hold what the agent, its screen or the
person's files said, and stays in the part's log and the run's evidence directory with everything
else that is not published. The codes are `not_selected`, `build_blocked`, `no_account`,
`agent_stopped`, `no_ledger`, `variable_missing`, `budget_held`, `ledger_unreadable`,
`login_files_unreadable`, `budget_short`, `upload_refused`, `upload_transferred_another_image`,
`launch_not_detected`, `resume_forks`, `reply_before_record`, `queued_prompt_not_during_turn`,
`not_pinned`, `environment_not_clear`, `environment_not_read`, `key_scan_incomplete`, `part_failed`,
`exited_after_outcome`, `installed_other_package` and `agent_stops` with the class of what stopped
the agent;
`launch_not_detected` carries the cause the check names (the launched execution ended, the host
announced other than one live instance, the instance names another package, is not integrated as a
native terminal or does not say its bridges are refused, or its binding cannot be read, is not a
native terminal's, names no launch profile or not the announced one, or is not counted live) and how many
live instances were announced. The record names a changed file in the
person's home only where it names a conversation a part of the record created or is one of the
files the entry names (`guarded`, `shared`, `recorded`, `append_only`, `line_files`); every other
change, an entry saying why a path could not be read among them, is counted under the part's
`person_home.others`, by category and by the listed directory it lies in. Any text naming a path
in the home names no other conversation, the rest of its line from the path on replaced where it
would, and a path built from the account name keeps `{user}` in its place. Every turn is charged
to the ledger `--turns` names before it is submitted; the harness holds the agent's budget for the
whole part, refuses a ledger it cannot read, and does not run a part the budget cannot cover.

In every part the session's shell searches only the run's link to the build, the run's links to its
runtimes and the system's directories. A part whose session searched or ran anything else, or whose
launch did not run the pinned file, did not test the pinned build, and is recorded as not run,
naming what ran.

Before the first agent, the harness starts one session the way a person does, `kr new` with their
own home, no agent and no choice of execution context, and checks that it names their login
keychain as its default. After that check and after each part it reads everything SecurityAgent,
which shows the system's keychain and authorisation dialogs, logged since a minute before its
previous look, and at the end the whole run, and says that nothing was, or stops at once with exit
status 3, as it does when the log cannot be read. After a part with a login it also reads what
coreauthd, which asks for a password or Touch ID, and tccd, which decides privacy permissions,
logged at every level about the run's programs (an entry naming the agent builds, a run's
directory, or a process number the part's sessions ran, which the driver's outcome lists however
the part ended), and stops the same way unless each tccd request about them was a preflight, which
answers from the stored decision and asks nobody, each coreauthd entry about them only created,
configured, released or evaluated a context not interactively, and coreauthd evaluated nothing
interactively for anyone during the part. It also stops when the part's outcome does not list its
process numbers. Each of these checks, the search of its evidence for a login's variable and the
look at SecurityAgent's log runs whatever another found before the run stops.
