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
#      RECORD lists. A build that is missing or different runs nothing, and says so.
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
# No agent signs in or starts a turn: each runs with its home inside the run's own directory, a
# keychain of the run's own as that home's default, and every proxy variable at a loopback port
# nothing listens on. The managed shell is the package KR_SHELL_PACKAGES names;
# scripts/build-shells.sh --zsh in the core checkout builds one.
#
# Usage: scripts/e2e-agents.sh --core DIR [--target-dir DIR] [--tools DIR]
#                              [--agent PACKAGE]... [--case PART]... [--write]
#
#   --core DIR        the core repository checkout whose host and driver are built and run
#   --target-dir DIR  the Cargo target directory to build in (default: CARGO_TARGET_DIR, or the
#                     checkout's own)
#   --tools DIR       where the agent builds are installed (default: KR_AGENTS_TOOLS)
#   --agent PACKAGE   run this package only, as publisher/plugin; repeat for several
#   --case PART       run this part only, such as 2b or 14.03a; repeat for several
#   --write           write the records into fixtures/agents
#
# It exits 0 when every part it ran passed, 1 when one failed or the host did not build, 2 when it
# was refused before running anything, and 3 when SecurityAgent logged anything during the run, or
# its log could not be read, at once: look at the screen, and answer no dialog there.
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
  "1||needs a vendor account and a spending bound, then the typed or bridge route|an account from the user|vendor account"
  "2a||needs a live conversation, which needs a vendor account|an account from the user|vendor account"
  "2b|an_agent_on_its_terminal_route_is_advertised_no_typed_capability_and_every_typed_action_is_refused|||"
  "2c||needs a binding, and the host hands no installed connector to a worker|the connector hand-over to workers|connector binding"
  "3||needs a vendor account and an approval a binding serves|an account from the user; the connector hand-over to workers|vendor account"
  "4||needs a vendor account and a typed route|an account from the user; the connector hand-over to workers|vendor account"
  "5a|a_control_daemon_crash_leaves_the_agent_and_its_local_terminal_running|||"
  "5b||the terminal route has no gateway, so it holds no gateway request state|the connector hand-over to workers|connector binding"
  "6a|a_running_agent_keeps_its_build_through_an_upgrade_and_the_newer_build_gets_the_terminal_route|||"
  "6b||needs a binding, to show each process bound to its own adapter and schema|the connector hand-over to workers|connector binding"
  "7||needs a vendor account, since a saved conversation needs a turn|an account from the user|vendor account"
  "8a|forged_titles_transcripts_identifiers_and_hook_input_leave_the_host_unchanged|||"
  "8b||needs a registration the host issued, to fail a forgery at the endpoint a legitimate one passes|the command integration|launch registration"
  "14.03a|a_path_typed_at_the_agent_is_terminal_input_that_reaches_its_composer|||"
  "14.03b||no method reads a draft's attachment state, and no adapter reports that an agent accepted one|device reads of drafts; the connector hand-over to workers|draft reads"
)

usage() {
  sed -n '36,49p' "${BASH_SOURCE[0]}" | sed 's/^# \{0,1\}//' >&2
  exit 2
}

core=""
target_dir="${CARGO_TARGET_DIR:-}"
tools="${KR_AGENTS_TOOLS:-}"
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

# Stops the run when SecurityAgent logged anything since $1, so a dialog one of its programs caused
# is never followed by another; otherwise says in the log that none opened. $2 names what ran.
no_dialog_since() {
  local since="$1" what="$2" logged now
  now="$(date '+%Y-%m-%d %H:%M:%S')"
  logged="$(security_agent_since "$since")"
  if [ -n "$logged" ]; then
    echo "$what: SecurityAgent's log from $since has entries, so a system dialog may have opened; stopping" >&2
    printf '%s\n' "$logged" | sed 's/^/  /' >&2
    exit 3
  fi
  echo "$what: SecurityAgent logged nothing from $since to $now, so no dialog opened"
}

# Each look at SecurityAgent's log starts a minute before the previous look began, so the looks
# overlap and none of the run falls between them; the last covers the whole run.
last_look="$run_began"
look_again() {
  local began
  began="$(date '+%Y-%m-%d %H:%M:%S')"
  no_dialog_since "$last_look" "$1"
  last_look="$(date -j -v-60S -f '%Y-%m-%d %H:%M:%S' "$began" '+%Y-%m-%d %H:%M:%S')"
}

# Compares the code a wheel installed under $1 with the wheel $2: every file the wheel's RECORD
# lists with a SHA-256 must be in the installation's site-packages with that digest. Prints how
# many files were compared, or what did not match.
verify_wheel() {
  local prefix="$1" wheel="$2" site list count others
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

# Once per run, before any agent: a session a person starts with their own home, and no agent,
# names their login keychain as its default. It is the first step of every record.
own_home_test="a_session_started_with_a_persons_own_home_keeps_their_login_keychain_as_its_default"
own_home_command="KR_AGENTS_RESULT=$evidence/own-home.jsonl KR_REQUIRE_AGENTS=1 KR_REQUIRE_SHELL_PACKAGES=1 $driver --exact $own_home_test --nocapture --test-threads=1"
began="$(date +%s)"
own_home_exit=0
KR_AGENTS_RESULT="$evidence/own-home.jsonl" KR_REQUIRE_AGENTS=1 KR_REQUIRE_SHELL_PACKAGES=1 \
  "$driver" --exact "$own_home_test" --nocapture --test-threads=1 >"$evidence/own-home.log" 2>&1 ||
  own_home_exit=$?
own_home_seconds=$(($(date +%s) - began))
look_again "own home"
own_home_error=""
own_home="{}"
if [ "$own_home_exit" -eq 0 ] &&
  own_home="$(jq -ec 'select(.part == "own-home" and .outcome == "passed") | .evidence' \
    "$evidence/own-home.jsonl" 2>/dev/null)"; then
  echo "own home: a session started with the person's own home and no agent names $(printf '%s' "$own_home" | jq -r '.default_keychain') as its default keychain, in the $(printf '%s' "$own_home" | jq -r '.worker_profile') context"
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
    if compared="$(verify_wheel "$tools/$(printf '%s' "$entry" | jq -r '.prefix')" "$pinned_file")"; then
      payload="; its installed code matches the $compared files the wheel's RECORD lists"
    else
      blocked="the code installed from $pinned_file differs from it: $compared (the tools directory)"
    fi
  fi
  manifest_version="$(jq -r '.version' "$manifest")"
  manifest_digest="$(digest_of "$manifest")"

  # The build as the driver reads it: paths made absolute, runtimes found, the package's actions.
  actions="$(jq '[.actions[].id]' "$manifest")"
  printf '%s' "$entry" | jq --arg tools "$tools" --argjson files "$files" --argjson actions "$actions" \
    --argjson newer_status "$newer_status" '{
      package, actions: $actions, application, version,
      prefix: ($tools + "/" + .prefix), pinned, sha256, command, arguments,
      runtime: [.runtime[] | $files[.]], environment, home, ready, harmless, composer,
      server, transcript,
      newer: (if .newer == null or $newer_status != true then null else
        {version: .newer.version, prefix: ($tools + "/" + .newer.prefix), pinned: .newer.pinned,
         sha256: .newer.sha256, ready: .newer.ready} end)
    }' >"$directory/build.json"

  steps="[$own_home_step]"
  number=1
  for spec in "${parts[@]}"; do
    part="${spec%%|*}"
    rest="${spec#*|}"
    test_name="${rest%%|*}"
    [ -n "$test_name" ] || continue
    selected_part "$part" || continue
    number=$((number + 1))
    if [ -n "$blocked" ]; then
      jq -cn --arg part "$part" --arg test "$test_name" --arg reason "$blocked" \
        '{part: $part, test: $test, outcome: "not_run", reason: $reason, evidence: {}}' >>"$results"
      continue
    fi
    log="$directory/$part.log"
    began="$(date +%s)"
    rc=0
    KR_AGENTS_BUILD="$directory/build.json" KR_AGENTS_GENERATION="$generation" \
      KR_AGENTS_RESULT="$results" KR_REQUIRE_AGENTS=1 KR_REQUIRE_SHELL_PACKAGES=1 \
      "$driver" --exact "$test_name" --nocapture --test-threads=1 >"$log" 2>&1 || rc=$?
    seconds=$(($(date +%s) - began))
    look_again "$package $version part $part"
    command_run="KR_AGENTS_BUILD=$directory/build.json KR_AGENTS_GENERATION=$generation KR_AGENTS_RESULT=$results KR_REQUIRE_AGENTS=1 KR_REQUIRE_SHELL_PACKAGES=1 $driver --exact $test_name --nocapture --test-threads=1"
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
        '{part: $part, test: $test, outcome: "not_run", reason: $reason, evidence: {exit: $exit}}' \
        >>"$results"
    elif ! jq -e --arg part "$part" 'select(.part == $part)' "$results" >/dev/null 2>&1; then
      # The test wrote no outcome: it failed, or it was skipped, which the driver refuses when
      # asked to run. Its own words say which.
      reason="$(grep -m 1 -A 1 'panicked at' "$log" | tail -1 || true)"
      [ -n "$reason" ] || reason="$(grep -m 1 -E '^skipping' "$log" || true)"
      [ -n "$reason" ] || reason="the part wrote no outcome and exited $rc"
      jq -cn --arg part "$part" --arg test "$test_name" --arg reason "$reason" --argjson exit "$rc" \
        '{part: $part, test: $test, outcome: "failed", reason: $reason, evidence: {exit: $exit}}' \
        >>"$results"
    elif [ "$rc" -ne 0 ]; then
      jq -cn --arg part "$part" --arg test "$test_name" --argjson exit "$rc" \
        '{part: $part, test: $test, outcome: "failed", reason: ("the part exited " + ($exit | tostring) + " after writing its outcome"), evidence: {exit: $exit}}' \
        >>"$results"
    else
      # What the host installed is what the part ran against, and it has to be this tree's package.
      installed="$(jq -c --arg part "$part" 'select(.part == $part) | .evidence.installed // null' "$results" | tail -1)"
      if [ "$(printf '%s' "$installed" | jq -r '.version // ""')" != "$manifest_version" ] ||
        [ "$(printf '%s' "$installed" | jq -r '.manifest_digest // ""')" != "$manifest_digest" ]; then
        jq -cn --arg part "$part" --arg test "$test_name" --arg version "$manifest_version" \
          --arg digest "$manifest_digest" --argjson installed "$installed" \
          '{part: $part, test: $test, outcome: "failed", reason: ("the host installed " + ($installed | tojson) + ", not this tree'"'"'s package " + $version + " with manifest " + $digest), evidence: {installed: $installed}}' \
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
  jq -n \
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
    --arg tools "$tools" --arg tools_real "$(cd "$tools" && pwd -P)" --arg home "${HOME:-}" \
    --slurpfile results "$results" '
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
        | (if $home == "" then . elif . == $home then "~" else split($home + "/") | join("~/") end)
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
      else
        {test: ("kr-e2e-agents --test cases " + $p.test), part: $p.part,
         source: "kalareach:tests/e2e/agents/tests/cases.rs", keyed_by: "attached_comment",
         outcome: $r.outcome}
        + (if $r.reason != null then {reason: $r.reason} else {} end)
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
    } | walk(local_paths) | .run.evidence_directory = $evidence' >"$directory/record.json"

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

  if [ "$write" -eq 1 ]; then
    destination="$root/fixtures/agents/$publisher/$plugin/$version/$platform.json"
    mkdir -p "$(dirname "$destination")"
    # The evidence directory is this machine's; the record keeps its name as the run stated it.
    cp "$directory/record.json" "$destination"
    echo "$package: wrote $destination"
  fi
done <<<"$packages"

echo
no_dialog_since "$run_began" "the whole run"
echo "$passed_count of $ran parts that ran passed; evidence in $evidence"
exit "$failed"
