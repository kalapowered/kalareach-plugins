#!/bin/bash
# Assembles a qualification record from the inputs of one run and prints it.
#
#   scripts/assemble-record.sh <inputs.json>
#
# The inputs are one JSON object with every parameter of `assemble_record` in
# scripts/record-redaction.jq: the run's identity, its steps, the parts table, the driver's result
# lines (`results`, an array with one object per outcome) and the package's manifest. The record is
# the one scripts/e2e-agents.sh writes, in the form fixtures/agents/README.md states, and is as
# redact_record publishes it. The run may come from any driver: nothing here starts an agent, a host
# or a network call.
#
# Exits 2 and prints nothing to standard output when jq is missing, the file is not a JSON object,
# or a parameter is missing from it.
set -euo pipefail

root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

# The parameters of `assemble_record`, in its order.
parameters=(
  package application version started finished evidence release plugins_commit core_commit
  toolchain entry steps parts selection manifest_version manifest_digest generation_identity
  payload own_home own_home_error evidence_real temporary temporary_real target target_real shells
  manifest pinned_status blocked newer_status tools tools_real home user login_parts results
)

refuse() {
  echo "assemble-record: $*" >&2
  exit 2
}

[ "$#" -eq 1 ] || refuse "usage: scripts/assemble-record.sh <inputs.json>"
command -v jq >/dev/null 2>&1 || refuse "jq is not installed"
[ -f "$1" ] || refuse "$1 is not a file"
jq -e 'type == "object"' "$1" >/dev/null 2>&1 || refuse "$1 is not a JSON object"

names="$(printf '%s\n' "${parameters[@]}" | jq -R . | jq -s -c .)"
missing="$(jq -r --argjson names "$names" '. as $inputs | $names[] | select($inputs[.] == null)' "$1")"
[ -z "$missing" ] || refuse "the inputs miss $(printf '%s' "$missing" | tr '\n' ' ')"

call=""
for name in "${parameters[@]}"; do
  call="${call:+$call; }\$inputs.$name"
done
jq -n -L "$root/scripts" --slurpfile inputs "$1" \
  "include \"record-redaction\"; \$inputs[0] as \$inputs | assemble_record($call)"
