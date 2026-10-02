#!/usr/bin/env bash
#
# Proves that this repository's .gitignore keeps the local workspace names out of every commit.
#
#   scripts/check-local-names.sh              check the .gitignore at HEAD and the tracked files
#   scripts/check-local-names.sh --self-test  prove each refusal on a repository planted with its defect
#
# The local workspace names are the instruction files and directories of the common coding
# assistants, at the root of the tree and below it. The check does two things:
#
#   - It puts the .gitignore from HEAD in a new repository of its own, creates each name there and
#     asks `git check-ignore` whether it is ignored. Git runs there with no configuration file, no
#     ignore file of this machine, an empty template and case-sensitive patterns, so nothing the
#     working tree or the person running the check holds, and no lower-case pattern on a
#     case-insensitive file system, can stand in for a pattern the file lacks. A name the file
#     lacks, a pattern that a later negation undoes and a missing .gitignore all refuse.
#   - It requires the local workspace section, from its "# Local workspace files" heading to the
#     next blank line, and alone it has to ignore every name, so a renamed heading, an empty
#     section and a pattern moved out of it all refuse. It refuses a tracked file that the
#     section's patterns match, at any depth, which a .gitignore does not stop.
#
# It reads the commit, never the working tree: uncommitted changes play no part.
set -euo pipefail

# The names a .gitignore has to ignore, relative to the root of a tree.
local_names=(
  AGENTS.md AGENTS-notes.md agents.md agents-notes.md docs/AGENTS.md
  CLAUDE.md CLAUDE-notes.md claude.md claude-notes.md docs/CLAUDE.md
  GEMINI.md GEMINI-notes.md gemini.md gemini-notes.md docs/GEMINI.md
  .claude/settings.json .codex/config.toml .agents/skills/example/SKILL.md .gemini/settings.json
  .cursor/rules/example.mdc .cursorrules .windsurf/rules/example.md .windsurfrules
  .aider.conf.yml .aider.chat.history.md .clinerules
  .github/copilot-instructions.md .github/instructions/example.instructions.md
  .github/prompts/example.prompt.md docs/.github/copilot-instructions.md
)

say() {
  printf 'check-local-names: %s\n' "$*"
}

# Runs git without the global or system configuration, without the ignore file Git reads by
# default and with case-sensitive patterns, so no ignore rule of this machine applies and a
# lower-case pattern cannot stand in for an upper-case one.
bare_git() {
  env GIT_CONFIG_GLOBAL=/dev/null GIT_CONFIG_NOSYSTEM=1 git -c core.excludesFile=/dev/null \
    -c core.ignoreCase=false "$@"
}

# Prints the local workspace section of a .gitignore: the lines from its "# Local workspace files"
# heading to the next blank line.
section_of() {
  awk '/^# Local workspace files$/ { found = 1 } found && /^[[:space:]]*$/ { exit } found { print }'
}

# Prints each local name the .gitignore at the given commit does not ignore, and each tracked path
# that the section's patterns match. Returns 1 when there is one of either.
check_commit() {
  local repository="$1" commit="$2" scratch name missing=() alone=() tracked status=0

  if ! bare_git -C "$repository" rev-parse --verify --quiet "$commit^{commit}" > /dev/null; then
    say "refused: $commit is not a commit of $repository"
    return 1
  fi

  scratch="$(mktemp -d "${TMPDIR:-/tmp}/kalareach-plugins-local-names.XXXXXX")"
  # shellcheck disable=SC2064
  trap "rm -rf '${scratch:?}'" EXIT
  bare_git init -q --template= "$scratch"
  if bare_git -C "$repository" cat-file -e "$commit:.gitignore" 2> /dev/null; then
    bare_git -C "$repository" show "$commit:.gitignore" > "$scratch/.gitignore"
  fi
  for name in "${local_names[@]}"; do
    mkdir -p "$scratch/$(dirname "$name")"
    : > "$scratch/$name"
    if ! bare_git -C "$scratch" check-ignore -q -- "$name"; then
      missing+=("$name")
    fi
  done

  # The section has to be there and has to carry every name by itself, and what the commit tracks
  # that its patterns match, at any depth, is decided by it alone: a tracked build file is not a
  # local workspace one.
  section_of < "$scratch/.gitignore" > "$scratch/section.ignore" 2> /dev/null || true
  tracked=""
  if [ -s "$scratch/section.ignore" ]; then
    cp "$scratch/section.ignore" "$scratch/.gitignore"
    for name in "${local_names[@]}"; do
      if ! bare_git -C "$scratch" check-ignore -q --no-index -- "$name"; then
        alone+=("$name")
      fi
    done
    tracked="$(bare_git -C "$repository" ls-tree -r -z --name-only "$commit" \
      | bare_git -C "$scratch" check-ignore --no-index --stdin -z | tr '\0' '\n' || true)"
  fi

  if [ ! -s "$scratch/section.ignore" ]; then
    say "refused: the .gitignore at $commit has no \"# Local workspace files\" section, a heading and the"
    say "lines that follow it up to a blank line"
    status=1
  fi
  if [ "${#alone[@]}" -ne 0 ]; then
    say "refused: the local workspace section at $commit does not ignore these names by itself:"
    printf '  %s\n' "${alone[@]}"
    status=1
  fi
  if [ "${#missing[@]}" -ne 0 ]; then
    say "refused: the .gitignore at $commit does not ignore these local workspace names:"
    printf '  %s\n' "${missing[@]}"
    status=1
  fi
  if [ -n "$tracked" ]; then
    say "refused: $commit tracks files that the .gitignore's local workspace section matches:"
    printf '%s\n' "$tracked" | sed 's/^/  /'
    status=1
  fi
  if [ "$status" -eq 0 ]; then
    say "the .gitignore at $commit ignores all ${#local_names[@]} local workspace names and tracks none"
  fi
  return "$status"
}

# The .gitignore section a repository carries, written to the path given.
write_section() {
  cat > "$1" << 'EOF'
# Local workspace files
AGENTS*.md
agents*.md
CLAUDE*.md
claude*.md
GEMINI*.md
gemini*.md
.claude/
.codex/
.agents/
.gemini/
.cursor/
.cursorrules
.windsurf/
.windsurfrules
.aider*
.clinerules
**/.github/copilot-instructions.md
**/.github/instructions/
**/.github/prompts/
EOF
}

fixture_git() {
  env GIT_CONFIG_GLOBAL=/dev/null GIT_CONFIG_NOSYSTEM=1 GIT_AUTHOR_NAME=fixture \
    GIT_AUTHOR_EMAIL=fixture@example.invalid GIT_COMMITTER_NAME=fixture \
    GIT_COMMITTER_EMAIL=fixture@example.invalid git "$@"
}

# The self-test: one fixture repository per refusal, each with its defect planted, and one that
# must pass.
self_test() {
  local work failures=0 directory output rc
  work="$(mktemp -d "${TMPDIR:-/tmp}/kalareach-plugins-local-names-test.XXXXXX")"
  # shellcheck disable=SC2064
  trap "rm -rf '${work:?}'" EXIT

  make_fixture() {
    directory="$work/$1"
    mkdir -p "$directory"
    fixture_git init -q -b main "$directory"
    printf '# fixture\n' > "$directory/README.md"
    write_section "$directory/.gitignore"
    fixture_git -C "$directory" add -A
    fixture_git -C "$directory" commit -q -m "Start the fixture"
  }

  commit_fixture() {
    fixture_git -C "$directory" add -A
    fixture_git -C "$directory" commit -q -m "$1"
  }

  # A case is a name, the status it must end with (pass or refuse) and a phrase its output must
  # contain. The fixture it runs against is the one made last.
  expect() {
    local case_name="$1" expected="$2" needle="$3"
    rc=0
    output="$(bash "$0" --repository "$directory" 2>&1)" || rc=$?
    if { [ "$expected" = pass ] && [ "$rc" -ne 0 ]; } \
      || { [ "$expected" = refuse ] && [ "$rc" -eq 0 ]; } \
      || ! printf '%s\n' "$output" | grep -qF -- "$needle"; then
      echo "self-test: $case_name FAILED (expected $expected, exit $rc)"
      printf '%s\n' "$output" | sed 's/^/    /'
      failures=$((failures + 1))
    else
      echo "self-test: $case_name ok"
    fi
  }

  make_fixture clean
  expect "a .gitignore with the section passes" pass "ignores all"

  make_fixture missing
  grep -v -e '^\.claude/$' -e '^CLAUDE' "$directory/.gitignore" > "$directory/.gitignore.next"
  mv "$directory/.gitignore.next" "$directory/.gitignore"
  commit_fixture "Drop two patterns"
  expect "a .gitignore that lacks a pattern is refused" refuse "does not ignore these local workspace names"

  make_fixture negated
  printf '!docs/AGENTS.md\n' >> "$directory/.gitignore"
  commit_fixture "Re-include one name"
  expect "a pattern that a later negation undoes is refused" refuse "docs/AGENTS.md"

  make_fixture absent
  fixture_git -C "$directory" rm -q .gitignore
  fixture_git -C "$directory" commit -q -m "Remove the .gitignore"
  expect "a repository with no .gitignore is refused" refuse "does not ignore these local workspace names"

  make_fixture lowercase
  grep -v -e '^CLAUDE' "$directory/.gitignore" > "$directory/.gitignore.next"
  mv "$directory/.gitignore.next" "$directory/.gitignore"
  commit_fixture "Drop the upper-case pattern"
  expect "a lower-case pattern cannot stand in for the upper-case one" refuse "CLAUDE.md"

  make_fixture heading
  sed 's/^# Local workspace files$/# Files for local use/' "$directory/.gitignore" > "$directory/.gitignore.next"
  mv "$directory/.gitignore.next" "$directory/.gitignore"
  printf 'notes\n' > "$directory/CLAUDE.md"
  fixture_git -C "$directory" add -f CLAUDE.md
  commit_fixture "Rename the heading and track an instruction file"
  expect "a renamed heading is refused" refuse "has no \"# Local workspace files\" section"

  make_fixture emptysection
  { printf '# Local workspace files\n\n'; sed '1d' "$directory/.gitignore"; } > "$directory/.gitignore.next"
  mv "$directory/.gitignore.next" "$directory/.gitignore"
  commit_fixture "Leave the section empty"
  expect "an empty section is refused although its patterns follow" refuse "does not ignore these names by itself"

  make_fixture moved
  grep -v -e '^\.codex/$' "$directory/.gitignore" > "$directory/.gitignore.next"
  mv "$directory/.gitignore.next" "$directory/.gitignore"
  printf '\n# Elsewhere\n.codex/\n' >> "$directory/.gitignore"
  commit_fixture "Move a pattern out of the section"
  expect "a pattern moved out of the section is refused" refuse "  .codex/config.toml"

  make_fixture machine
  printf '# nothing\n' > "$directory/.gitignore"
  commit_fixture "Empty the .gitignore"
  mkdir -p "$work/xdg/git"
  printf 'AGENTS.md\nCLAUDE.md\n.claude/\n' > "$work/xdg/git/ignore"
  XDG_CONFIG_HOME="$work/xdg" expect "an ignore file of the machine cannot stand in" refuse \
    "  AGENTS.md"

  for planted in "root=AGENTS-project.md" "nested=src/AGENTS.md" "directory=.claude/other.json" \
    "exact=CLAUDE.md"; do
    make_fixture "tracked-${planted%%=*}"
    mkdir -p "$directory/$(dirname "${planted#*=}")"
    printf 'notes\n' > "$directory/${planted#*=}"
    fixture_git -C "$directory" add -f "${planted#*=}"
    fixture_git -C "$directory" commit -q -m "Track an instruction file"
    expect "a tracked file the section matches (${planted%%=*}) is refused" refuse \
      "tracks files that the .gitignore's local workspace section matches"
  done

  make_fixture uncommitted
  rm "$directory/.gitignore"
  expect "an uncommitted change plays no part" pass "ignores all"

  if [ "$failures" -ne 0 ]; then
    echo "self-test: $failures case(s) failed"
    return 1
  fi
  echo "self-test: every case ok"
}

case "${1:-}" in
  --self-test) self_test ;;
  --repository)
    check_commit "${2:?--repository needs a path}" HEAD
    ;;
  "")
    check_commit "$(cd "$(dirname "$0")/.." && pwd)" HEAD
    ;;
  *)
    echo "usage: $0 [--self-test | --repository <path>]" >&2
    exit 2
    ;;
esac
