#!/usr/bin/env bash
# The qualification cases for the bundled agent connectors, run against a real host.
#
# Section 12 asks for eight cases per bundled agent, run against the build its connector table is
# qualified for and recorded per operating system and architecture. Each case has one or more parts
# (fixtures/agents/README.md lists them). This runs the parts a host shows on an agent's terminal
# route, through the qualification driver in the core repository (tests/e2e/agents), and records
# every part, run or not, in one record per package under fixtures/agents.
#
#   1. It builds the host (kr, kr-attach-guard, kr-controller, kr-worker, kr-hook) and the driver in
#      the core checkout --core names, or finds them already built there, and names the commit.
#   2. It checks each agent build in the tools directory against the SHA-256 fixtures/agents/
#      builds.json pins, and a build installed from a wheel against every file digest the wheel's
#      RECORD lists and its launcher against the RECORD the wheel's own distribution installed. A
#      build that is missing or different runs nothing, and says so.
#   3. Once, with no agent, it checks that a session a person starts with their own home, in the
#      execution context the host gives it, names their login keychain as its default.
#   4. For each package, and each selected part the driver has a test for, the driver starts a host
#      from those binaries on the internal disk, pairs an owner device, installs the package from a
#      copy of snapshots/development on that device's confirmation, starts the agent by typing its
#      command at the prompt of a managed shell, and checks the part and its control. A part whose
#      session searched or ran anything but the pinned build, its runtime, the run's own and the
#      system's did not test the build, and is recorded as not run, naming what ran. A part whose
#      host installed anything but this tree's package failed.
#   5. After that check and after each part it reads everything SecurityAgent, which shows the
#      system's keychain and authorisation dialogs, logged since a minute before its previous look,
#      and at the end the whole run: nothing, which it says, or it stops, as it does when the log
#      cannot be read.
#   6. It prints one line per part: passed, failed, or not run with the reason and who can change it.
#   7. It assembles each package's record, and writes it into fixtures/agents with --write.
#
# The driver runs with a cleared environment (the home, the user, the temporary directory, the
# system's PATH and the harness's inputs). A part that needs no login runs the agent with its home
# inside the run's own directory, a keychain of the run's own as that home's default, and every
# proxy variable at a loopback port nothing listens on, and starts no turn. Parts 1, 2a, 3, 4 and 7
# need the person's vendor login: they run only for a build whose list entry names an approved
# login (`account`), as that entry says (whose home, the one variable a login may be, handed to the
# driver on a pipe, the arguments), and each turn they start is charged to the ledger --turns names
# before it is submitted; the harness holds the agent's budget through the part and does not run a
# part its budget cannot cover. After such a part it also reads what coreauthd and tccd logged about
# the run's programs, and searches its evidence for a variable's value and for the values of the
# other keys its own shell holds (provider_secret_values), which no session is given: the driver is
# handed those on a pipe too, to search the run's directory for before it removes it, and before a
# part with a login the driver's check that a launched session exports none of an entry's cleared
# variables (provider_keys_test) must have passed. The managed shell is the
# package KR_SHELL_PACKAGES names; scripts/build-shells.sh --zsh in the core checkout builds one.
#
# Usage: scripts/e2e-agents.sh --core DIR [--target-dir DIR] [--tools DIR] [--turns FILE]
#                              [--agent PACKAGE]... [--case PART]... [--write]
#
#   --core DIR        the core repository checkout whose host and driver are built and run
#   --target-dir DIR  the Cargo target directory to build in (default: CARGO_TARGET_DIR, or the
#                     checkout's own)
#   --tools DIR       where the agent builds are installed (default: KR_AGENTS_TOOLS)
#   --turns FILE      the ledger the parts with a login charge their turns to, kept across runs
#                     (default: KR_AGENTS_TURNS); without it those parts do not run
#   --agent PACKAGE   run this package only, as publisher/plugin; repeat for several
#   --case PART       run this part only, such as 2b or 14.03a; repeat for several
#   --write           write the records into fixtures/agents
#
# It exits 0 when every part it ran passed, 1 when one failed, the host did not build or a login's
# value was found in its evidence, 2 when it was refused before running anything, and 3 when
# SecurityAgent logged anything during the run, coreauthd or tccd logged a request about its
# programs, or a log could not be read, at once: look at the screen, and answer no dialog there.
set -euo pipefail

# Every comparison here is of ASCII identifiers and digests.
export LC_ALL=C

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
builds_file="$root/fixtures/agents/builds.json"
platform="macos-aarch64"

# The parts, in the order a record lists them, each as "part|test|reason|owner|needs". A part with a
# test runs through the driver; a part without one is recorded as not run, with its reason, who can
# change that, and what it needs.
parts=(
  "1|a_device_prompts_the_agent_and_adds_an_image_and_the_local_terminal_shows_the_same_execution|||"
  "2a|slash_commands_interrupts_queued_prompts_and_steering_each_work_from_a_device|||"
  "2b|an_agent_on_its_terminal_route_is_advertised_no_typed_capability_and_every_typed_action_is_refused|||"
  "2c|under_a_binding_a_supported_action_is_permitted_and_an_unsupported_one_is_refused|||"
  "3|a_local_and_a_remote_answer_raced_to_one_approval_resolve_it_once|||"
  "4|a_disconnection_after_the_agent_took_a_prompt_leaves_one_reply_and_no_duplicate_work|||"
  "5a|a_control_daemon_crash_leaves_the_agent_and_its_local_terminal_running|||"
  "5b||waits for the command integration: a session made through kr new gets no command integration, so no launch has a gateway holding request state|the command integration's landing|command integration"
  "6a|a_running_agent_keeps_its_build_through_an_upgrade_and_the_newer_build_gets_the_terminal_route|||"
  "6b||waits for the command integration: a build outside the qualified range can be refused only a typed route, and no launch has one without the integration|the command integration's landing|command integration"
  "7|a_second_process_on_the_same_saved_conversation_is_another_execution_not_merged|||"
  "8a|forged_titles_transcripts_identifiers_and_hook_input_leave_the_host_unchanged|||"
  "8b||waits for the command integration: no launch has a registration the host issued, against which a forgery could fail where it passes|the command integration's landing|command integration"
  "14.03a|a_path_typed_at_the_agent_is_terminal_input_that_reaches_its_composer|||"
  "14.03b||no method reads a draft's attachment state, and no adapter reports that an agent accepted one|device reads of drafts; the connector hand-over to workers|draft reads"
)

# The most turns each part with a login can start, which the harness reserves before it runs one.
login_turns() {
  case "$1" in
    1) echo 2 ;;
    2a) echo 5 ;;
    3) echo 2 ;;
    4) echo 2 ;;
    7) echo 2 ;;
    *) echo 0 ;;
  esac
}
# The parts that run with the login, for the record's writer (scripts/record-redaction.jq).
login_parts='["1", "2a", "3", "4", "7"]'

# The environment the driver runs with, and nothing else (env -i): whatever else this shell
# holds, a key among it, reaches no program the driver starts. A login's one variable goes to the
# driver on a pipe instead, which is empty once the driver has read it.
driver_environment=("HOME=${HOME:-}" "USER=${USER:-}" "LOGNAME=${LOGNAME:-}" "TMPDIR=${TMPDIR:-/tmp}"
  "PATH=/usr/bin:/bin:/usr/sbin:/sbin" "LANG=en_US.UTF-8")
for name in KR_SHELL_PACKAGES KR_SHELL_PREFIX RUST_BACKTRACE XDG_RUNTIME_DIR DBUS_SESSION_BUS_ADDRESS; do
  if [ -n "${!name:-}" ]; then driver_environment+=("$name=${!name}"); fi
done

usage() {
  sed -n '/^# Usage:/,/^# It exits/p' "${BASH_SOURCE[0]}" | sed 's/^# \{0,1\}//' >&2
  exit 2
}

core=""
target_dir="${CARGO_TARGET_DIR:-}"
tools="${KR_AGENTS_TOOLS:-}"
turns="${KR_AGENTS_TURNS:-}"
write=0
agents=()
cases=()
while [ "$#" -gt 0 ]; do
  case "$1" in
    --core)
      [ "$#" -ge 2 ] || usage
      core="$2"
      shift 2
      ;;
    --target-dir)
      [ "$#" -ge 2 ] || usage
      target_dir="$2"
      shift 2
      ;;
    --tools)
      [ "$#" -ge 2 ] || usage
      tools="$2"
      shift 2
      ;;
    --turns)
      [ "$#" -ge 2 ] || usage
      turns="$2"
      shift 2
      ;;
    --agent)
      [ "$#" -ge 2 ] || usage
      agents+=("$2")
      shift 2
      ;;
    --case)
      [ "$#" -ge 2 ] || usage
      cases+=("$2")
      shift 2
      ;;
    --write)
      write=1
      shift
      ;;
    *) usage ;;
  esac
done

refuse() {
  echo "$1" >&2
  exit 2
}

[ -n "$core" ] || refuse "--core names no core checkout"
[ -f "$core/Cargo.toml" ] && [ -d "$core/tests/e2e/agents" ] ||
  refuse "$core is not a core checkout with the qualification driver (tests/e2e/agents)"
core="$(cd "$core" && pwd)"
[ -n "$tools" ] || refuse "--tools or KR_AGENTS_TOOLS names no directory of agent builds"
[ -d "$tools" ] || refuse "the tools directory $tools does not exist"
tools="$(cd "$tools" && pwd)"
tools_real="$(cd "$tools" && pwd -P)"
[ -n "$target_dir" ] || target_dir="$core/target"
command -v jq >/dev/null || refuse "jq is required to read the build list and write the records"
[ "$(uname -s)-$(uname -m)" = "Darwin-arm64" ] ||
  refuse "the build list names builds for $platform, and this is $(uname -s) $(uname -m)"
[ "$(jq -r '.platform' "$builds_file")" = "$platform" ] ||
  refuse "$builds_file is not a build list for $platform"

known_part() {
  local wanted="$1" entry
  for entry in "${parts[@]}"; do
    [ "${entry%%|*}" = "$wanted" ] && return 0
  done
  return 1
}
for wanted in ${cases[@]+"${cases[@]}"}; do
  known_part "$wanted" || refuse "$wanted is not a part of a qualification case"
done
packages="$(jq -r '.builds[].package' "$builds_file")"
for wanted in ${agents[@]+"${agents[@]}"}; do
  printf '%s\n' "$packages" | grep -qx -- "$wanted" || refuse "$wanted is not a package in $builds_file"
done

selected_part() {
  local part="$1" wanted
  [ "${#cases[@]}" -eq 0 ] && return 0
  for wanted in "${cases[@]}"; do
    [ "$wanted" = "$part" ] && return 0
  done
  return 1
}
selected_agent() {
  local package="$1" wanted
  [ "${#agents[@]}" -eq 0 ] && return 0
  for wanted in "${agents[@]}"; do
    [ "$wanted" = "$package" ] && return 0
  done
  return 1
}

# Every artefact of this run goes into a new directory on the internal disk, named with its links
# resolved, as the processes that write into it name it.
evidence="$(cd "$(mktemp -d "${TMPDIR:-/tmp}/kalareach-agents-XXXXXX")" && pwd -P)"
temporary="${TMPDIR:-/tmp}"
temporary="${temporary%/}"
temporary_real="$(cd "$temporary" && pwd -P)"
started="$(date -u '+%Y-%m-%dT%H:%M:%SZ')"
# When the run began, as `log show` reads a time.
run_began="$(date '+%Y-%m-%d %H:%M:%S')"

commit_of() {
  local id modified=false
  id="$(git -C "$1" rev-parse HEAD)"
  [ -z "$(git -C "$1" status --porcelain --untracked-files=no)" ] || modified=true
  printf '{"id":"%s","modified":%s}' "$id" "$modified"
}
plugins_commit="$(commit_of "$root")"
core_commit="$(commit_of "$core")"

echo "kalareach agent qualification"
echo "  plugins commit: $(printf '%s' "$plugins_commit" | jq -r '.id')"
echo "  host commit: $(printf '%s' "$core_commit" | jq -r '.id') (core checkout $core)"
echo "  machine: $(sw_vers -productVersion 2>/dev/null || true) $(uname -sr) $(uname -m)"
echo "  taken at: $started"
echo "  evidence: $evidence"
echo

# The host and the driver, built once before anything runs, so a build failure is not reported as
# a part that failed. The driver launches the host binaries from beside its own test executable.
if ! (cd "$core" &&
  CARGO_TARGET_DIR="$target_dir" cargo build --locked -p kr-cli -p kr-controller -p kr-worker \
    -p kr-hook --bins &&
  CARGO_TARGET_DIR="$target_dir" cargo test --locked -p kr-e2e-agents --test cases --no-run \
    --message-format=json-render-diagnostics >"$evidence/driver.json") >"$evidence/build.log" 2>&1; then
  tail -20 "$evidence/build.log" >&2
  echo "the host or the driver did not build, so no part ran" >&2
  exit 1
fi
driver="$(jq -r 'select(.reason == "compiler-artifact" and .target.name == "cases" and .profile.test == true) | .executable' \
  "$evidence/driver.json" | tail -1)"
[ -x "$driver" ] || {
  echo "the driver's test executable was not found in the build's output" >&2
  exit 1
}
toolchain="$(cd "$core" && jq -n --arg rustc "$(rustc -V)" --arg cargo "$(cargo -V)" '{rustc: $rustc, cargo: $cargo}')"

# The generation every package is installed from: the committed development generation, copied to
# the internal disk.
generation="$evidence/generation"
cp -R "$root/snapshots/development" "$generation"

# The executable each named runtime is on this machine, as JSON. The driver links each into a
# directory of the run's own, so the session's PATH names no directory another installation shares.
runtime_files() {
  local names="$1" name found files="{}"
  for name in $names; do
    found="$(command -v "$name" 2>/dev/null)" || return 1
    files="$(printf '%s' "$files" | jq --arg name "$name" --arg file "$found" '. + {($name): $file}')"
  done
  printf '%s' "$files"
}

# What SecurityAgent, which shows the system's keychain and authorisation dialogs and runs only to
# show one, logged since $1, at every level. A `log show` that fails has said nothing, and what it
# printed is returned as an entry would be, so the run stops rather than reading its silence as none.
security_agent_since() {
  local out
  if ! out="$(/usr/bin/log show --style compact --info --debug --start "$1" \
    --predicate 'process == "SecurityAgent"' 2>&1)"; then
    printf 'the system log could not be read: %s\n' "$out"
    return 0
  fi
  printf '%s\n' "$out" | grep 'SecurityAgent\[' || true
}

# Fails (status 3) when SecurityAgent logged anything since $1, and the caller stops the run, so a
# dialog one of its programs caused is never followed by another; otherwise says in the log that
# none opened. $2 names what ran.
no_dialog_since() {
  local since="$1" what="$2" logged now
  now="$(date '+%Y-%m-%d %H:%M:%S')"
  logged="$(security_agent_since "$since")"
  if [ -n "$logged" ]; then
    echo "$what: SecurityAgent's log from $since has entries, so a system dialog may have opened; stopping" >&2
    printf '%s\n' "$logged" | sed 's/^/  /' >&2
    return 3
  fi
  echo "$what: SecurityAgent logged nothing from $since to $now, so no dialog opened"
}

# Each look at SecurityAgent's log starts a minute before the previous look began, so the looks
# overlap and none of the run falls between them; the last covers the whole run.
last_look="$run_began"
look_again() {
  local began
  began="$(date '+%Y-%m-%d %H:%M:%S')"
  no_dialog_since "$last_look" "$1" || return 3
  last_look="$(date -j -v-60S -f '%Y-%m-%d %H:%M:%S' "$began" '+%Y-%m-%d %H:%M:%S')"
}

# Sorts what tccd and coreauthd logged about this run's programs into what can ask someone and what
# cannot. tccd handles a request on one thread from its REQUEST line to its REPLY line, and its
# AUTHREQ_CTX line names the service and whether the request is a preflight, which answers from the
# stored decision and never asks anyone. coreauthd asks only while it evaluates a policy or an
# access control for a context interactively; creating a context (its creation and the proxy
# returned to the client), setting an option on it, an evaluation made not interactive, releasing
# it (its deallocation, or the proxy interrupted when the client's connection ends), and a
# connection's own records ask nothing. An evaluation is taken as not interactive only in the one
# form a check whether a policy can be evaluated makes: options that are all numbers, option 1000
# set to 1 among them, and no UI delegate as the entry's last words. Any other evaluation counts
# as one that may ask, whoever made it, since coreauthd names its caller only by a process number,
# which a process that ended between two looks never left, and no other form the same entry also
# has excuses it. The first file names this run's programs, one form per line; the second is the
# log, whose continuation lines are joined to their entry. Prints one line per tccd request about
# them ("request <msgID> preflight=<yes|no> <service> <result>"), one per coreauthd record of
# those kinds about them ("context <entry>"), and every other entry about them, and every
# evaluation that may ask, marked "coreauthd" or "outside" (a tccd entry outside any request).
# shellcheck disable=SC2016
tcc_requests='
FNR == NR { if (length($0) > 0) forms[++count] = $0; next }
function ours(line,   i) {
  for (i = 1; i <= count; i++) if (index(line, forms[i]) > 0) return 1
  return 0
}
# One entry of the log, its continuation lines joined to it.
function handle(line,   thread, connection, id, context, evaluation, quiet) {
  if (match(line, /(tccd|coreauthd)\[[0-9]+:[0-9a-f]+\]/) == 0) return
  thread = substr(line, RSTART, RLENGTH)
  connection = index(line, "[com.apple.xpc:connection]") > 0
  if (index(thread, "coreauthd") == 1) {
    if (connection) return
    evaluation = line ~ /evaluatePolicy:/ || line ~ /evaluateAccessControl/
    quiet = line ~ /evaluatePolicy:[0-9]+ options:\{( +[0-9]+ = [0-9]+;)* *\}, uiDelegate:0 on ContextProxy\[[0-9:]+\] rid:[0-9]+ *$/ &&
      index(line, " 1000 = 1;") > 0
    # An evaluation that may ask counts whoever made it, before any other form of the entry.
    if (evaluation && !quiet) { print "coreauthd " line; return }
    if (!ours(line)) return
    if (line ~ /ContextProxy\[[0-9:]+\] created for Context/ ||
        line ~ /LAContext\] returning ContextProxy\[[0-9:]+\] on client: / ||
        line ~ /setServerPropertyForOption:[0-9]+ value:[0-9]+ on ContextProxy\[/ || quiet ||
        line ~ /ContextProxy\[[0-9:]+\] deallocated/ ||
        line ~ /LAContext\] ContextProxy\[[0-9:]+\] interrupted *$/ ||
        line ~ /Untracking proxy:ContextProxy\[/) print "context " line
    else print "coreauthd " line
    return
  }
  if (match(line, /REQUEST: .*msgID=[0-9.]+/)) {
    id = substr(line, RSTART, RLENGTH); sub(/.*msgID=/, "", id); current[thread] = id
  }
  id = current[thread]
  if (id == "") { if (ours(line) && !connection) print "outside " line; return }
  if (match(line, /AUTHREQ_CTX: msgID=[0-9.]+, function=[^,]*, service=[A-Za-z]+, preflight=[a-z]+/)) {
    context = substr(line, RSTART, RLENGTH)
    service[id] = context; sub(/.*service=/, "", service[id]); sub(/,.*/, "", service[id])
    preflight[id] = context; sub(/.*preflight=/, "", preflight[id])
  }
  if (match(line, /AUTHREQ_RESULT: msgID=[0-9.]+, authValue=[0-9]+, authReason=[0-9]+/)) {
    result[id] = substr(line, RSTART, RLENGTH); sub(/.*authValue/, "authValue", result[id])
  }
  if (ours(line)) about[id] = 1
  if (match(line, /REPLY: .*msgID=[0-9.]+/)) current[thread] = ""
}
/^[0-9][0-9][0-9][0-9]-[0-9][0-9]-[0-9][0-9] / { if (entry != "") handle(entry); entry = $0; next }
{ if (entry != "") entry = entry " " $0 }
END {
  if (entry != "") handle(entry)
  for (id in about) print "request " id " preflight=" preflight[id] " " service[id] " " result[id]
}
'

# After a part with a login: what coreauthd, which asks for a password or Touch ID, and tccd, which
# decides privacy permissions, logged since $1 about this run's programs, at every level. A line is
# about them when it names the agent builds or a run's directory, or one of the process numbers $3
# the part's sessions ran, in the forms these services name a caller by. Every tccd request about
# them must be a preflight and every coreauthd record about them a context's own (tcc_requests);
# anything else about them, or a log it cannot read or sort, fails the check (status 3), and the
# caller stops the run, as SecurityAgent does.
no_prompt_since() {
  local since="$1" what="$2" pids="$3" out pid found asked preflights contexts
  if ! out="$(/usr/bin/log show --style compact --info --debug --start "$since" \
    --predicate 'process == "coreauthd" OR process == "tccd"' 2>&1)"; then
    echo "$what: the system log could not be read: $out" >&2
    return 3
  fi
  if ! found="$(awk "$tcc_requests" <(
    printf '%s\n' "$tools" "$tools_real" "$temporary_real/krm-" "$temporary/krm-"
    for pid in $pids; do printf '%s\n' "ContextProxy[$pid:" "(pid $pid)" "pid=$pid,"; done
  ) <(printf '%s\n' "$out"))"; then
    echo "$what: what coreauthd and tccd logged since $since could not be sorted" >&2
    return 3
  fi
  preflights="$(printf '%s\n' "$found" | grep -c '^request [0-9.]* preflight=yes ' || true)"
  contexts="$(printf '%s\n' "$found" | grep -c '^context ' || true)"
  asked="$(printf '%s\n' "$found" | grep . | grep -v -e '^request [0-9.]* preflight=yes ' -e '^context ' || true)"
  if [ -n "$asked" ]; then
    echo "$what: coreauthd or tccd logged something about this run's programs since $since that may have asked someone; stopping" >&2
    printf '%s\n' "$asked" | sed 's/^/  /' >&2
    return 3
  fi
  echo "$what: since $since tccd logged $preflights request(s) about this run's programs, each a preflight, and coreauthd $contexts record(s) of their contexts and nothing else; neither asks anyone"
}

# After a part that carried a login's variable: the value is in none of the files this run has
# written, the part's log and outcome among them, once the driver has ended. The value goes to grep
# on a pipe; only file names come back. A value found, or a file grep could not read, fails the
# check (status 1), and the caller stops the run.
key_not_in() {
  local directory="$1" variable="$2" what="$3" found rc=0
  if ! printenv "$variable" >/dev/null; then
    echo "$what: $variable is not in this shell's environment, so no value was carried" >&2
    return 1
  fi
  found="$(grep -r -l -F -f <(printenv "$variable") "$directory")" || rc=$?
  if [ "$rc" -gt 1 ]; then
    echo "$what: $directory could not be searched whole for the value of $variable (grep exited $rc); stopping" >&2
    return 1
  fi
  if [ "$rc" -eq 0 ]; then
    echo "$what: the value of $variable is in these files, which it must never reach; stopping" >&2
    printf '%s\n' "$found" | sed 's/^/  /' >&2
    return 1
  fi
  echo "$what: the value of $variable is in none of this run's files, the part's log included"
}

# The names of the variables whose values are keys, tokens or secrets: a name of the shell's own
# environment that one of these globs names, and that is not the product's own (KR_*).
secret_names=('*_API_KEY' '*_API_TOKEN' '*_ACCESS_TOKEN' '*_AUTH_TOKEN' '*_TOKEN' '*_SECRET' '*_SECRET_KEY'
  '*_ACCESS_KEY' '*_PASSWORD')

# The values of the keys this shell holds, one on each line, but the one variable $1 names (the
# login's own, which is carried and searched for on its own): the variables of its environment that
# secret_names names and whose value is sixteen characters or more on one line. They go to the
# caller's pipe and are never printed or kept.
provider_secret_values() {
  local skip="${1:-}" name value pattern
  while IFS= read -r name; do
    [ "$name" != "$skip" ] || continue
    case "$name" in KR_*) continue ;; esac
    for pattern in "${secret_names[@]}"; do
      # shellcheck disable=SC2254 # the pattern is the glob
      case "$name" in
        $pattern)
          value="${!name}"
          if [ "${#value}" -ge 16 ] && [[ $value != *$'\n'* ]]; then printf '%s\n' "$value"; fi
          break
          ;;
      esac
    done
  done < <(compgen -e)
}

# After a part with a login: the value of no other key this shell holds is in any file this run has
# written, the part's log and outcome among them, once the driver has ended. The values go to grep
# on a pipe; only file names come back. A value found, or a file grep could not read, fails the check
# (status 1), and the caller stops the run. $3 names the login's own variable, searched for by
# key_not_in.
others_not_in() {
  local directory="$1" what="$2" skip="$3" found rc=0 count
  count="$(provider_secret_values "$skip" | wc -l | tr -d ' ')"
  if [ "$count" -eq 0 ]; then
    echo "$what: this shell holds no other key to search this run's files for"
    return 0
  fi
  found="$(grep -r -l -F -f <(provider_secret_values "$skip") "$directory")" || rc=$?
  if [ "$rc" -gt 1 ]; then
    echo "$what: $directory could not be searched whole for the values of the other keys this shell holds (grep exited $rc); stopping" >&2
    return 1
  fi
  if [ "$rc" -eq 0 ]; then
    echo "$what: the value of a key this shell holds is in these files, which no key may reach; stopping" >&2
    printf '%s\n' "$found" | sed 's/^/  /' >&2
    return 1
  fi
  echo "$what: the values of the $count other keys this shell holds are in none of this run's files, the part's log included"
}

# The strings of the credentials files of an agent whose login is kept in files of its own, sixteen
# characters or more, on one line each: those of every credentials file in its data directory, the
# login in use and the others. The configuration's key strings and any a second refresh leaves between
# what is read here before and after a part are searched for by the driver, which reads them as TOML
# and at every look while the part runs, in what the agent wrote and in this run's evidence so far.
# They go to the caller's memory or to grep on a pipe and are never printed or kept; the status is
# non-zero where a file cannot be read as JSON or none holds such a string.
confinement_strings() {
  local entry_json="$1" data file out values=""
  data="$HOME/$(printf '%s' "$entry_json" | jq -r '.account.confinement.data')" || return 1
  for file in "$data"/credentials/*.json; do
    [ -f "$file" ] || continue
    out="$(jq -r '.. | strings | select(length >= 16 and (contains("\n") | not))' "$file")" || return 1
    values+="$out"$'\n'
  done
  values="$(printf '%s' "$values" | sed '/^$/d')" || return 1
  [ -n "$values" ] || return 1
  printf '%s\n' "$values"
}

# After a part of an agent whose login is kept in files of its own: none of the strings of the
# credentials file in use, as it was before the part ($4) and as it is now (the agent may have
# refreshed it), is in any file of this run's evidence, the part's log included. The strings go to
# grep on a pipe and are never printed or kept; only file names come back. A string found, a file
# grep could not read, or a login file that cannot be found or read, before or after, fails the
# check (status 1), and the caller stops the run.
secrets_not_in() {
  local directory="$1" what="$2" entry_json="$3" before="$4" now found rc=0
  if ! now="$(confinement_strings "$entry_json")"; then
    echo "$what: the login's files cannot be found or read, so the evidence cannot be searched for their strings" >&2
    return 1
  fi
  if [ -z "$before" ]; then
    echo "$what: the login's files were not read before the part, so the evidence cannot be searched for the strings they held then" >&2
    return 1
  fi
  found="$(grep -r -l -F -f <(printf '%s\n%s\n' "$before" "$now") "$directory")" || rc=$?
  if [ "$rc" -gt 1 ]; then
    echo "$what: $directory could not be searched whole for the strings of the login's files (grep exited $rc); stopping" >&2
    return 1
  fi
  if [ "$rc" -eq 0 ]; then
    echo "$what: a string of the login's files is in these files, which they must never reach; stopping" >&2
    printf '%s\n' "$found" | sed 's/^/  /' >&2
    return 1
  fi
  echo "$what: no string of the login's files, as they were before the part or after it, is in any file of this run's evidence, the part's log included"
}

# Compares the code a wheel installed under $1 with the wheel $2: every file the wheel's RECORD
# lists with a SHA-256 must be in the installation's site-packages with that digest, and the
# launcher $3 a person types must be the one the installer wrote, as the installation's own RECORD
# names it, starting that installation's interpreter. Prints how many files were compared, or what
# did not match.
verify_wheel() {
  local prefix="$1" wheel="$2" launcher="$3" site list count others recorded wanted actual dist record
  site="$(find "$prefix/lib" -maxdepth 2 -type d -name site-packages | head -1)"
  [ -n "$site" ] || {
    echo "no site-packages under $prefix"
    return 1
  }
  list="$(mktemp "$evidence/record-XXXXXX")"
  unzip -p "$wheel" '*.dist-info/RECORD' |
    awk -F, '{ h = $2; sub(/^sha256=/, "", h) } $2 ~ /^sha256=/ && length(h) == 64 && h ~ /^[0-9a-f]+$/ { print h "  " $1 }' >"$list"
  others="$(unzip -p "$wheel" '*.dist-info/RECORD' | awk -F, '$2 != "" && $2 !~ /^sha256=[0-9a-f]+$/' | wc -l | tr -d ' ')"
  count="$(wc -l <"$list" | tr -d ' ')"
  if [ "$others" -ne 0 ] || [ "$count" -eq 0 ]; then
    echo "the wheel's RECORD lists $others digests in a form this harness does not compare"
    return 1
  fi
  if ! (cd "$site" && shasum -a 256 -c --quiet "$list") >"$list.out" 2>&1; then
    head -3 "$list.out" | tr '\n' ';'
    return 1
  fi
  if [ "$(head -1 "$prefix/bin/$launcher")" != "#!$prefix/bin/python" ]; then
    echo "$prefix/bin/$launcher does not start the installation's own interpreter"
    return 1
  fi
  # The launcher belongs to the pinned wheel's own distribution, as that distribution's installed
  # RECORD says, and to no other.
  dist="$(unzip -Z1 "$wheel" | grep -E '^[^/]+[.]dist-info/RECORD$' | head -1)"
  dist="${dist%/RECORD}"
  if [ -z "$dist" ] || [ ! -f "$site/$dist/RECORD" ]; then
    echo "the installation holds no RECORD of the wheel's own distribution"
    return 1
  fi
  others="$(for record in "$site"/*.dist-info/RECORD; do
    [ "$record" = "$site/$dist/RECORD" ] && continue
    awk -F, -v name="../../../bin/$launcher" '$1 == name' "$record"
  done | wc -l | tr -d ' ')"
  if [ "$others" -ne 0 ]; then
    echo "another distribution's RECORD also names $prefix/bin/$launcher"
    return 1
  fi
  recorded="$(awk -F, -v name="../../../bin/$launcher" '$1 == name { print $2 }' "$site/$dist/RECORD" | head -1)"
  wanted="${recorded#sha256=}"
  while [ $((${#wanted} % 4)) -ne 0 ]; do wanted="$wanted="; done
  wanted="$(printf '%s' "$wanted" | tr '_-' '/+' | base64 -D 2>/dev/null | xxd -p -c 64)"
  actual="$(shasum -a 256 "$prefix/bin/$launcher" | cut -d ' ' -f 1)"
  if [ -z "$recorded" ] || [ "$wanted" != "$actual" ]; then
    echo "$prefix/bin/$launcher is not the launcher the wheel's installed RECORD names"
    return 1
  fi
  echo "$count"
}

# A file's SHA-256, or nothing when it is not there.
digest_of() {
  [ -f "$1" ] || return 0
  shasum -a 256 "$1" | cut -d ' ' -f 1
}

failed=0
ran=0
passed_count=0
written=()

# The budget lock this run holds, if any: taken before a part with a login, released when the
# driver ends, and released on the way out whatever ends the run.
budget_lock=""
release_budget() {
  if [ -n "$budget_lock" ]; then
    rmdir "$budget_lock" 2>/dev/null || true
    budget_lock=""
  fi
}
trap release_budget EXIT

# The driver's check that a launched session exports no provider key (see the loop below).
provider_keys_test="a_session_exports_no_provider_key_but_those_the_build_list_sets_and_a_harmless_variable_passes"

# Once per run, before any agent: a session a person starts with their own home, and no agent,
# names their login keychain as its default. It is the first step of every record.
own_home_test="a_session_started_with_a_persons_own_home_keeps_their_login_keychain_as_its_default"
own_home_command="env -i <driver environment> KR_AGENTS_RESULT=$evidence/own-home.jsonl KR_REQUIRE_AGENTS=1 KR_REQUIRE_SHELL_PACKAGES=1 $driver --exact $own_home_test --nocapture --test-threads=1"
began="$(date +%s)"
own_home_exit=0
env -i "${driver_environment[@]}" KR_AGENTS_RESULT="$evidence/own-home.jsonl" KR_REQUIRE_AGENTS=1 KR_REQUIRE_SHELL_PACKAGES=1 \
  "$driver" --exact "$own_home_test" --nocapture --test-threads=1 >"$evidence/own-home.log" 2>&1 ||
  own_home_exit=$?
own_home_seconds=$(($(date +%s) - began))
look_again "own home" || exit 3
own_home_error=""
own_home="{}"
if [ "$own_home_exit" -eq 0 ] &&
  own_home="$(jq -ec 'select(.part == "own-home" and .outcome == "passed") | .evidence' \
    "$evidence/own-home.jsonl" 2>/dev/null)"; then
  echo "own home: a session started with the person's own home and no agent names $(printf '%s' "$own_home" | jq -r '.default_keychain') as its default keychain, in the $(printf '%s' "$own_home" | jq -r '.worker_profile') context, and security show-keychain-info exits $(printf '%s' "$own_home" | jq -r '.show_keychain_info_status') there"
else
  own_home="{}"
  own_home_error="$(grep -m 1 -A 1 'panicked at' "$evidence/own-home.log" | tail -1 || true)"
  [ -n "$own_home_error" ] || own_home_error="the check exited $own_home_exit and wrote no outcome"
  echo "own home: failed ($own_home_error)"
  failed=1
fi
own_home_step="$(jq -cn --arg command "$own_home_command" --argjson exit "$own_home_exit" \
  --argjson seconds "$own_home_seconds" --arg error "$own_home_error" \
  '{number: 1, group: "own-home", what: "a session started with the person'"'"'s own home names their login keychain",
    command: $command, needs: ["KR_SHELL_PACKAGES"], exit: $exit, seconds: $seconds, log: "own-home.log",
    error: (if $error == "" then null else $error end)}')"
echo

while IFS= read -r package; do
  [ -n "$package" ] || continue
  selected_agent "$package" || continue
  entry="$(jq --arg package "$package" '.builds[] | select(.package == $package)' "$builds_file")"
  application="$(printf '%s' "$entry" | jq -r '.application')"
  version="$(printf '%s' "$entry" | jq -r '.version')"
  publisher="${package%%/*}"
  plugin="${package#*/}"
  manifest="$root/plugins/$publisher/$plugin/plugin.json"
  directory="$evidence/$plugin"
  mkdir -p "$directory"
  results="$directory/results.jsonl"
  : >"$results"
  steps="[]"

  # Why this build's parts cannot run here, when they cannot.
  blocked=""
  pinned_file="$tools/$(printf '%s' "$entry" | jq -r '.prefix')/$(printf '%s' "$entry" | jq -r '.pinned')"
  pinned_digest="$(digest_of "$pinned_file")"
  wanted_digest="$(printf '%s' "$entry" | jq -r '.sha256')"
  if [ -z "$pinned_digest" ]; then
    blocked="the pinned build $version is not installed at $pinned_file (the tools directory)"
  elif [ "$pinned_digest" != "$wanted_digest" ]; then
    blocked="$pinned_file is not the pinned build $version: its SHA-256 is $pinned_digest (the tools directory)"
  fi
  newer_status="null"
  if [ "$(printf '%s' "$entry" | jq '.newer != null')" = "true" ]; then
    newer_file="$tools/$(printf '%s' "$entry" | jq -r '.newer.prefix')/$(printf '%s' "$entry" | jq -r '.newer.pinned')"
    if [ "$(digest_of "$newer_file")" = "$(printf '%s' "$entry" | jq -r '.newer.sha256')" ]; then
      newer_status="true"
    else
      newer_status="false"
    fi
  fi
  runtimes="$(printf '%s' "$entry" | jq -r '.runtime | join(" ")')"
  files="{}"
  if [ -z "$blocked" ] && ! files="$(runtime_files "$runtimes")"; then
    blocked="the build runs on $runtimes, which is not on this machine's PATH (the machine)"
  fi
  # A build installed from a wheel runs the code the wheel installed, which is compared with it.
  payload=""
  if [ -z "$blocked" ] && [ "${pinned_file##*.}" = "whl" ]; then
    if compared="$(verify_wheel "$tools/$(printf '%s' "$entry" | jq -r '.prefix')" "$pinned_file" \
      "$(printf '%s' "$entry" | jq -r '.command')")"; then
      payload="; its installed code matches the $compared files the wheel's RECORD lists, and its launcher the RECORD of the wheel's own distribution, the only one naming it"
    else
      blocked="the code installed from $pinned_file differs from it: $compared (the tools directory)"
    fi
  fi
  manifest_version="$(jq -r '.version' "$manifest")"
  manifest_digest="$(digest_of "$manifest")"

  # The build as the driver reads it: paths made absolute, runtimes found, the package's actions.
  actions="$(jq '[.actions[]]' "$manifest")"
  printf '%s' "$entry" | jq --arg tools "$tools" --arg root "$root" --argjson files "$files" --argjson actions "$actions" \
    --argjson newer_status "$newer_status" '{
      package, actions: $actions, application, version,
      prefix: ($tools + "/" + .prefix), pinned, sha256, command, launch, arguments,
      runtime: [.runtime[] | $files[.]], environment, home, ready, harmless, composer,
      server, transcript, no_account, no_newer,
      account: (if .account.confinement then .account | .confinement.profile = ($root + "/" + .confinement.profile)
                else .account end),
      newer: (if .newer == null or $newer_status != true then null else
        {version: .newer.version, prefix: ($tools + "/" + .newer.prefix), pinned: .newer.pinned,
         sha256: .newer.sha256, ready: .newer.ready} end)
    }' >"$directory/build.json"

  steps="[$own_home_step]"
  number=1
  stopped=""
  stopped_part=""

  # Before any part with a login of an agent whose entry names an account: a session made from an
  # environment that holds the keys a person's shell holds exports none of the entry's cleared
  # variables but those the entry sets itself, a harmless variable passes through the same route, and
  # the same environment with nothing taken out is refused by name. It starts no agent and no turn.
  if [ -z "$blocked" ] && [ "$(printf '%s' "$entry" | jq '.account != null')" = "true" ]; then
    wants_login=0
    for spec in "${parts[@]}"; do
      part="${spec%%|*}"
      rest="${spec#*|}"
      test_name="${rest%%|*}"
      if [ -n "$test_name" ] && selected_part "$part" && [ "$(login_turns "$part")" -gt 0 ]; then wants_login=1; fi
    done
    if [ "$wants_login" -eq 1 ]; then
      keys_log="$directory/provider-keys.log"
      if env -i "${driver_environment[@]}" "KR_AGENTS_BUILD=$directory/build.json" KR_REQUIRE_AGENTS=1 \
        KR_REQUIRE_SHELL_PACKAGES=1 "$driver" --exact "$provider_keys_test" --nocapture --test-threads=1 \
        >"$keys_log" 2>&1; then
        echo "$package $version: a session made from an environment that holds the keys a person's shell holds exports none of the entry's cleared variables but those the entry sets, a harmless variable passes, and the same environment with nothing taken out is refused by name"
      else
        keys_reason="$(grep -m 1 -A 1 'panicked at' "$keys_log" | tail -1 || true)"
        echo "$package $version: the provider-key check failed (${keys_reason:-no reason given}), so no part with a login runs and no turn was spent; stopping" >&2
        exit 1
      fi
      look_again "$package $version provider keys" || exit 3
    fi
  fi
  for spec in "${parts[@]}"; do
    part="${spec%%|*}"
    rest="${spec#*|}"
    test_name="${rest%%|*}"
    [ -n "$test_name" ] || continue
    selected_part "$part" || continue
    number=$((number + 1))
    # What a part that did not run says of why, for a record: the failure as a code and what varies
    # in it, since the reason's own text can hold a path or a name of the person's. Only a part with
    # a login has one.
    need="$(login_turns "$part")"
    if [ -n "$blocked" ]; then
      jq -cn --arg part "$part" --arg test "$test_name" --arg reason "$blocked" --argjson need "$need" \
        '{part: $part, test: $test, outcome: "not_run", reason: $reason,
          evidence: (if $need > 0 then {failure_codes: [{code: "build_blocked"}]} else {} end)}' >>"$results"
      continue
    fi
    secrets_before=""
    # A part with a login runs only for a build whose entry names one, while its agent has not been
    # stopped, and while its budget can cover the most turns the part starts.
    if [ "$need" -gt 0 ]; then
      login_reason=""
      login_code='[]'
      if [ "$(printf '%s' "$entry" | jq '.account == null')" = "true" ]; then
        login_reason="$(printf '%s' "$entry" | jq -r '.no_account // "the build list names no approved login for this agent (the user)"')"
        login_code='[{"code": "no_account"}]'
      elif [ -n "$stopped" ]; then
        login_reason="the agent stopped after $stopped"
        login_code="$(jq -cn --arg after "$stopped_part" '[{code: "agent_stopped", after: $after}]')"
      elif [ -z "$turns" ]; then
        login_reason="no ledger was named for its turns, so none may start (--turns)"
        login_code='[{"code": "no_ledger"}]'
      elif wanted="$(printf '%s' "$entry" | jq -r '.account.variable // ""')" && [ -n "$wanted" ] &&
        ! printenv "$wanted" >/dev/null; then
        login_reason="its login is the variable $wanted, which this shell's environment does not hold (the user)"
        login_code="$(jq -cn --arg variable "$wanted" '[{code: "variable_missing", variable: $variable}]')"
      elif [ "$(printf '%s' "$entry" | jq '.account.confinement != null')" = "true" ] &&
        ! secrets_before="$(confinement_strings "$entry")"; then
        login_reason="the login's files cannot be read, so the evidence could not be searched for their strings (the user)"
        login_code='[{"code": "login_files_unreadable"}]'
      else
        budget="$(printf '%s' "$entry" | jq -r '.account.budget')"
        limit="$(printf '%s' "$entry" | jq -r '.account.turns')"
        # The budget is held for the whole part: no other run can spend from it meanwhile.
        if ! mkdir "$turns.$budget.lock" 2>/dev/null; then
          login_reason="another run holds the $budget budget ($turns.$budget.lock)"
          login_code="$(jq -cn --arg budget "$budget" '[{code: "budget_held", budget: $budget}]')"
        else
          budget_lock="$turns.$budget.lock"
          spent=0
          if [ -e "$turns" ] && ! spent="$(jq -e --arg budget "$budget" '.budgets[$budget] // [] | length' "$turns")"; then
            login_reason="the ledger $turns cannot be read, so what the $budget budget has left is not known"
            login_code="$(jq -cn --arg budget "$budget" '[{code: "ledger_unreadable", budget: $budget}]')"
          elif [ $((limit - spent)) -lt "$need" ]; then
            login_reason="the $budget budget has $((limit - spent)) of its $limit turns left, fewer than the $need part $part can start (the ledger)"
            login_code="$(jq -cn --arg budget "$budget" --arg part "$part" --argjson left "$((limit - spent))" \
              --argjson limit "$limit" --argjson need "$need" \
              '[{code: "budget_short", budget: $budget, left: $left, limit: $limit, need: $need, part: $part}]')"
          fi
          [ -z "$login_reason" ] || release_budget
        fi
      fi
      if [ -n "$login_reason" ]; then
        jq -cn --arg part "$part" --arg test "$test_name" --arg reason "$login_reason" --argjson codes "$login_code" \
          '{part: $part, test: $test, outcome: "not_run", reason: $reason, needs: ["vendor account"],
            evidence: {failure_codes: $codes}}' >>"$results"
        continue
      fi
    fi
    log="$directory/$part.log"
    began="$(date +%s)"
    part_began="$(date '+%Y-%m-%d %H:%M:%S')"
    rc=0
    variable=""
    [ "$need" -gt 0 ] && variable="$(printf '%s' "$entry" | jq -r '.account.variable // ""')"
    login_variable_note=""
    [ "$need" -le 0 ] || login_variable_note=" KR_AGENTS_SCAN_FD=4 (the values of the other keys this shell holds, on a pipe)"
    inputs=("KR_AGENTS_BUILD=$directory/build.json" "KR_AGENTS_GENERATION=$generation"
      "KR_AGENTS_RESULT=$results" "KR_REQUIRE_AGENTS=1" "KR_REQUIRE_SHELL_PACKAGES=1"
      "KR_AGENTS_TURNS=$turns" "KR_AGENTS_KEY_SCAN=$directory/key-scan.jsonl")
    if [ -n "$variable" ]; then
      # The login's value goes on a pipe the driver reads once, and nowhere else; the values of the
      # other keys this shell holds go on another, for the driver to search the run's directory for.
      env -i "${driver_environment[@]}" "${inputs[@]}" KR_AGENTS_KEY_FD=3 KR_AGENTS_SCAN_FD=4 \
        "$driver" --exact "$test_name" --nocapture --test-threads=1 >"$log" 2>&1 \
        3< <(printenv "$variable") 4< <(provider_secret_values "$variable") || rc=$?
    elif [ "$need" -gt 0 ]; then
      env -i "${driver_environment[@]}" "${inputs[@]}" KR_AGENTS_SCAN_FD=4 \
        "$driver" --exact "$test_name" --nocapture --test-threads=1 >"$log" 2>&1 \
        4< <(provider_secret_values "") || rc=$?
    else
      env -i "${driver_environment[@]}" "${inputs[@]}" \
        "$driver" --exact "$test_name" --nocapture --test-threads=1 >"$log" 2>&1 || rc=$?
    fi
    seconds=$(($(date +%s) - began))
    release_budget
    # Every check runs whatever another found, and then a request that may have asked someone
    # stops the run with 3, a value where it must never be with 1.
    checked=0
    dialog=0
    look_again "$package $version part $part" || dialog=3
    if [ "$need" -gt 0 ]; then
      if [ -n "$variable" ]; then
        key_not_in "$evidence" "$variable" "$package $version part $part" || checked=1
      fi
      others_not_in "$evidence" "$package $version part $part" "$variable" || checked=1
      if [ "$(printf '%s' "$entry" | jq '.account.confinement != null')" = "true" ]; then
        secrets_not_in "$evidence" "$package $version part $part" "$entry" "$secrets_before" || checked=1
      fi
      # The process numbers the part's sessions ran, from the driver's own outcome, which has them
      # however the part ended; without them what coreauthd logged about the part cannot be told.
      if jq -e --arg part "$part" 'select(.part == $part and (.evidence.provenance.pids | type) == "array")' \
        "$results" >/dev/null 2>&1; then
        pids="$(jq -r --arg part "$part" 'select(.part == $part) | .evidence.provenance.pids // [] | .[]' \
          "$results" | tr '\n' ' ')"
        no_prompt_since "$part_began" "$package $version part $part" "$pids" || dialog=3
      else
        echo "$package $version part $part: the driver wrote no outcome with the process numbers the part ran, so what coreauthd logged about them cannot be told apart; stopping" >&2
        dialog=3
      fi
    fi
    [ "$dialog" -eq 0 ] || exit "$dialog"
    [ "$checked" -eq 0 ] || exit "$checked"
    if [ "$need" -gt 0 ]; then
      # The agent stops when its part says so, and when a part that did not pass shows no answer
      # only its model could have given: its login is then not established.
      if jq -e --arg part "$part" 'select(.part == $part and .evidence.stop_agent == true)' "$results" >/dev/null 2>&1; then
        stopped="part $part: $(jq -r --arg part "$part" 'select(.part == $part) | .reason' "$results" | tail -1)"
        stopped_part="$part"
      elif ! jq -e --arg part "$part" 'select(.part == $part and (.outcome == "passed" or .evidence.login_held == true))' \
        "$results" >/dev/null 2>&1; then
        stopped="part $part, which ended without an answer only the agent's model could have given, so its login is not established"
        stopped_part="$part"
      fi
    fi
    # The ledger's path is the person's own and says nothing of the part, so the command names it as such.
    shown_inputs="${inputs[*]}"
    [ -z "$turns" ] || shown_inputs="${shown_inputs//"$turns"/<ledger>}"
    command_run="env -i <driver environment> $shown_inputs${variable:+ KR_AGENTS_KEY_FD=3 (the value of $variable on a pipe)}${login_variable_note} $driver --exact $test_name --nocapture --test-threads=1"
    steps="$(printf '%s' "$steps" | jq --argjson number "$number" --arg part "$part" \
      --arg command "$command_run" --argjson exit "$rc" --argjson seconds "$seconds" \
      --arg log "$plugin/$part.log" \
      '. + [{number: $number, group: "agents", what: ("part " + $part), command: $command,
             needs: ["KR_SHELL_PACKAGES"], exit: $exit, seconds: $seconds, log: $log,
             error: null}]')"
    if ! jq -e --arg part "$part" 'select(.part == $part)' "$results" >/dev/null 2>&1 &&
      pinned_not="$(grep -m 1 -o 'not the pinned build: .*' "$log")"; then
      # The session searched or ran something other than the build under test, so the part did
      # not test it.
      jq -cn --arg part "$part" --arg test "$test_name" --arg reason "$pinned_not" --argjson exit "$rc" \
        --argjson need "$need" \
        '{part: $part, test: $test, outcome: "not_run", reason: $reason,
          evidence: ({exit: $exit} + (if $need > 0 then {failure_codes: [{code: "not_pinned"}]} else {} end))}' \
        >>"$results"
    elif ! jq -e --arg part "$part" 'select(.part == $part)' "$results" >/dev/null 2>&1; then
      # The test wrote no outcome: it failed, or it was skipped, which the driver refuses when
      # asked to run. Its own words say which.
      reason="$(grep -m 1 -A 1 'panicked at' "$log" | tail -1 || true)"
      [ -n "$reason" ] || reason="$(grep -m 1 -E '^skipping' "$log" || true)"
      [ -n "$reason" ] || reason="the part wrote no outcome and exited $rc"
      jq -cn --arg part "$part" --arg test "$test_name" --arg reason "$reason" --argjson exit "$rc" \
        --argjson need "$need" \
        '{part: $part, test: $test, outcome: "failed", reason: $reason,
          evidence: ({exit: $exit} + (if $need > 0 then {failure_codes: [{code: "part_failed"}]} else {} end))}' \
        >>"$results"
    elif [ "$rc" -ne 0 ]; then
      # A part that measured a failure it can describe writes it, with what it observed, and then
      # exits non-zero; that line stands. Anything else that exited non-zero after writing its
      # outcome failed on the way.
      # A driver-written not_run for a session that ran something other than the build, or whose
      # shell exports a variable the build list clears or could not be read, stands too: the driver
      # names each by its code.
      if ! jq -L "$root/scripts" -e --arg part "$part" 'include "record-redaction"; driver_line_stands($part)' \
        "$results" >/dev/null 2>&1; then
        jq -cn --arg part "$part" --arg test "$test_name" --argjson exit "$rc" --argjson need "$need" \
          '{part: $part, test: $test, outcome: "failed", reason: ("the part exited " + ($exit | tostring) + " after writing its outcome"),
            evidence: ({exit: $exit} + (if $need > 0 then {failure_codes: [{code: "exited_after_outcome"}]} else {} end))}' \
          >>"$results"
      fi
    else
      # What the host installed is what the part ran against, and it has to be this tree's package.
      installed="$(jq -c --arg part "$part" 'select(.part == $part) | .evidence.installed // null' "$results" | tail -1)"
      if [ "$(printf '%s' "$installed" | jq -r '.version // ""')" != "$manifest_version" ] ||
        [ "$(printf '%s' "$installed" | jq -r '.manifest_digest // ""')" != "$manifest_digest" ]; then
        jq -cn --arg part "$part" --arg test "$test_name" --arg version "$manifest_version" \
          --arg digest "$manifest_digest" --argjson installed "$installed" --argjson need "$need" \
          '{part: $part, test: $test, outcome: "failed", reason: ("the host installed " + ($installed | tojson) + ", not this tree'"'"'s package " + $version + " with manifest " + $digest),
            evidence: ({installed: $installed} + (if $need > 0 then {failure_codes: [{code: "installed_other_package"}]} else {} end))}' \
          >>"$results"
      fi
    fi
  done

  # The record: every part, the run's identity, the build and the package it is for.
  finished="$(date -u '+%Y-%m-%dT%H:%M:%SZ')"
  parts_json="$(for spec in "${parts[@]}"; do printf '%s\n' "$spec"; done |
    jq -R 'split("|") | {part: .[0], test: .[1], reason: .[2], owner: .[3], needs: .[4]}' | jq -s '.')"
  selection="[]"
  for spec in "${parts[@]}"; do
    part="${spec%%|*}"
    rest="${spec#*|}"
    if [ -n "${rest%%|*}" ] && selected_part "$part"; then
      selection="$(printf '%s' "$selection" | jq -c --arg part "$part" '. + [$part]')"
    fi
  done
  jq -n -L "$root/scripts" \
    --arg package "$package" --arg application "$application" --arg version "$version" \
    --arg started "$started" --arg finished "$finished" --arg evidence "$evidence" \
    --arg release "$(sw_vers -productVersion 2>/dev/null || true), Darwin $(uname -r)" \
    --argjson plugins_commit "$plugins_commit" --argjson core_commit "$core_commit" \
    --argjson toolchain "$toolchain" --argjson entry "$entry" --argjson steps "$steps" \
    --argjson parts "$parts_json" --argjson selection "$selection" \
    --arg manifest_version "$manifest_version" \
    --arg manifest_digest "$manifest_digest" \
    --arg payload "$payload" --argjson own_home "$own_home" --arg own_home_error "$own_home_error" \
    --arg evidence_real "$evidence" --arg temporary "$temporary" --arg temporary_real "$temporary_real" \
    --arg target "$target_dir" --arg target_real "$(cd "$target_dir" && pwd -P)" \
    --arg shells "${KR_SHELL_PACKAGES:-}" \
    --argjson manifest "$(cat "$manifest")" \
    --arg pinned_status "$([ -z "$blocked" ] && echo installed || echo not_installed)" \
    --arg blocked "$blocked" --argjson newer_status "$newer_status" \
    --arg tools "$tools" --arg tools_real "$tools_real" --arg home "${HOME:-}" \
    --arg user "$(id -un)" --argjson login_parts "$login_parts" \
    --slurpfile results "$results" '
    include "record-redaction";
    def counts($tests): reduce ("passed", "failed", "ignored", "not_run", "not_built", "known_difference") as $o
      ({}; . + {($o): ([$tests[] | select(.outcome == $o)] | length)});
    # A path in the evidence directory, the temporary directory, the build'"'"'s target directory, the
    # managed shell'"'"'s packages, the tools directory or the home directory of whoever ran this is
    # written relative to it; a system or runtime executable is named as it is, since which one ran
    # is the evidence.
    def local_paths: if type == "string" then
        (split($evidence_real) | join("<evidence>"))
        | (split($temporary_real) | join("<tmp>")) | (split($temporary) | join("<tmp>"))
        | (split($target_real) | join("<target>")) | (split($target) | join("<target>"))
        | (if $shells == "" then . else split($shells) | join("<shells>") end)
        | (split($tools_real) | join("<tools>")) | (split($tools) | join("<tools>"))
        | (if $home == "" then . elif . == $home then "~"
           else split($home + "/") | join("~/")
             | gsub(($home | literal_pattern) + "(?![A-Za-z0-9._-])"; "~") end)
      else . end;
    def verdict($tests): if any($tests[]; .outcome == "failed") then "failed"
      elif any($tests[]; .outcome == "passed") then "passed" else "not_run" end;
    def declared: ($manifest.attachments // null) as $a |
      if $a == null then null
      elif $a.insertion == "upstream_upload" then "typed_submission"
      elif $a.insertion == "native_composer" then "verified_composer_insertion"
      else $a.insertion end;
    ($steps | map(select(.group == "agents")) | map({key: (.what | ltrimstr("part ")), value: .number}) | from_entries) as $step_of |
    ($steps | map(select(.group == "agents")) | map({key: (.what | ltrimstr("part ")), value: .command}) | from_entries) as $command_of |
    [ $parts[] | . as $p |
      ([$results[] | select(.part == $p.part)] | last) as $r |
      if $p.test == "" then
        {test: ("part " + $p.part), part: $p.part, source: "kalareach-plugins:fixtures/agents/README.md",
         keyed_by: "case_table", outcome: "not_run",
         reason: ($p.reason + " (" + $p.owner + ")"),
         needs: [$p.needs]}
      elif $r == null then
        {test: ("kr-e2e-agents --test cases " + $p.test), part: $p.part,
         source: "kalareach:tests/e2e/agents/tests/cases.rs", keyed_by: "attached_comment",
         outcome: "not_run", reason: "not selected in this run"}
        + (if $entry.account != null and any($login_parts[]; . == $p.part)
           then {evidence: {failure_codes: [{code: "not_selected"}]}} else {} end)
      else
        {test: ("kr-e2e-agents --test cases " + $p.test), part: $p.part,
         source: "kalareach:tests/e2e/agents/tests/cases.rs", keyed_by: "attached_comment",
         outcome: $r.outcome}
        + (if $r.reason != null then {reason: $r.reason} else {} end)
        + (if $r.needs != null then {needs: $r.needs} else {} end)
        + (if $step_of[$p.part] != null then
             {runs: [{step: $step_of[$p.part], outcome: $r.outcome}],
              command: $command_of[$p.part]}
           else {} end)
        + {evidence: $r.evidence}
      end
    ] as $tests |
    ($tests | map(select(.part | startswith("14.03") | not))) as $case_tests |
    ($tests | map(select(.part | startswith("14.03")))) as $attachment_tests |
    {
      schema: "kalareach.conformance/1",
      extension: "kalareach.agent-qualification/1",
      repository: "kalareach-plugins",
      run: {
        started: $started, finished: $finished, platform: "macos",
        system: {os: "macos", release: $release, arch: "aarch64", target: "aarch64-apple-darwin"},
        commit: $plugins_commit,
        host: {repository: "kalareach", commit: $core_commit},
        toolchain: $toolchain,
        terminal_profile: {profile: "kr-vt/1", term: "ghostty"},
        packages: [{name: $package, version: $manifest_version, manifest_digest: $manifest_digest,
                    generation: "snapshots/development"}],
        applications: ([{id: $application, version: $version, status: $pinned_status,
                         url: $entry.source, sha256: $entry.sha256, build: "darwin-arm64",
                         reason: (if $blocked == "" then "the build the table is pinned to" + $payload else $blocked end)}]
          + (if $entry.newer == null then [] else
              [{id: $application, version: $entry.newer.version,
                status: (if $newer_status == true then "installed" else "not_installed" end),
                url: $entry.newer.source, sha256: $entry.newer.sha256, build: "darwin-arm64",
                reason: "the newer build the upgrade part moves to"}] end)),
        selection: $selection,
        all_terminals: false,
        evidence_directory: $evidence,
        own_home: $own_home
      },
      steps: $steps,
      identifiers: {
        "KR-REQ-12.32": {family: "requirement", verdict: verdict($case_tests),
                         counts: counts($case_tests), tests: $case_tests},
        "KR-REQ-14.03": {family: "requirement", verdict: verdict($attachment_tests),
                         counts: counts($attachment_tests), tests: $attachment_tests}
      },
      attachment_paths: (
        [$manifest.actions[] | select(.effect == "upstream.prompt" or .effect == "upstream.attachment") |
          {operation: .id, declared: declared, source: "plugin.json", package_digest: $manifest_digest}]
        + [{operation: "terminal", declared: null, source: "plugin.json", package_digest: $manifest_digest}]),
      failures_outside_identifiers: (if $own_home_error == "" then [] else
        [{test: "kr-e2e-agents --test cases a_session_started_with_a_persons_own_home_keeps_their_login_keychain_as_its_default",
          source: "kalareach:tests/e2e/agents/tests/cases.rs", step: 1, reason: $own_home_error}] end),
      known_differences: [],
      problems: [],
      warnings: [],
      summary: {
        identifiers: 2,
        passed: ([verdict($case_tests), verdict($attachment_tests)] | map(select(. == "passed")) | length),
        failed: ([verdict($case_tests), verdict($attachment_tests)] | map(select(. == "failed")) | length),
        known_difference: 0,
        not_run: ([verdict($case_tests), verdict($attachment_tests)] | map(select(. == "not_run")) | length),
        known_differences: 0,
        tests: counts($tests),
        failed_steps: [$steps[] | select(.exit != 0) | .number]
      }
    } | walk(local_paths) | .run.evidence_directory = $evidence
      # What the record may not name of the person: record-redaction.jq.
      | redact_record($user; $entry + {actions: [$manifest.actions[].id], grants: [$manifest.capabilities[].capability]}; $login_parts)' >"$directory/record.json"

  # One line per part, in the record's order.
  while IFS= read -r line; do
    printf '%s %s %s\n' "$package" "$version" "$line"
  done < <(jq -r '.identifiers[].tests[] |
      "part " + .part + ": " + (if .outcome == "not_run" then "not run: " + .reason else .outcome end)
      + (if .outcome == "failed" then " (" + (.reason // "") + ")" else "" end)' \
      "$directory/record.json")
  ran=$((ran + $(jq '[.identifiers[].tests[] | select(.outcome != "not_run")] | length' "$directory/record.json")))
  passed_count=$((passed_count + $(jq '[.identifiers[].tests[] | select(.outcome == "passed")] | length' "$directory/record.json")))
  if [ "$(jq '[.identifiers[].tests[] | select(.outcome == "failed")] | length' "$directory/record.json")" -gt 0 ]; then
    failed=1
  fi

  written+=("$publisher/$plugin/$version")
done <<<"$packages"

echo
# The whole run is read once more before any record is written, so a run that stops here writes
# none.
no_dialog_since "$run_began" "the whole run" || exit 3
if [ "$write" -eq 1 ]; then
  for record in ${written[@]+"${written[@]}"}; do
    destination="$root/fixtures/agents/$record/$platform.json"
    mkdir -p "$(dirname "$destination")"
    # The evidence directory is this machine's; the record keeps its name as the run stated it.
    cp "$evidence/$(printf '%s' "$record" | cut -d / -f 2)/record.json" "$destination"
    echo "wrote $destination"
  done
fi
echo "$passed_count of $ran parts that ran passed; evidence in $evidence"
exit "$failed"
