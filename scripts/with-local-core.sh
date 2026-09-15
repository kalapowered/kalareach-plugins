#!/usr/bin/env bash
#
# Runs a command with the kalareach core repository resolved from a local checkout.
#
#   scripts/with-local-core.sh cargo test
#   KALAREACH_CORE=/path/to/kalareach scripts/with-local-core.sh cargo run -p kalareach-catalogue -- validate
#
# The pipeline depends on kr-plugin-sdk by Git URL and exact revision, which is what pins the
# package contract this catalogue is built against. When the SDK and the catalogue change together,
# the revision is in a local checkout before it is anywhere else. This rewrites the fetch URL for
# the duration of one command so Cargo reads that checkout.
#
# Cargo.toml and Cargo.lock are untouched: they keep the canonical URL and revision, so what this
# builds is what a fetch from the published repository builds.

set -euo pipefail

if [ "$#" -eq 0 ]; then
    echo "usage: $0 <command> [args...]" >&2
    exit 2
fi

repository_root="$(cd "$(dirname "$0")/.." && pwd)"
core="${KALAREACH_CORE:-$(dirname "$repository_root")/kalareach}"

if [ ! -d "$core/.git" ] && [ ! -f "$core/.git" ]; then
    echo "$core is not a kalareach checkout; set KALAREACH_CORE" >&2
    exit 1
fi

core="$(cd "$core" && pwd)"

exec env \
    CARGO_NET_GIT_FETCH_WITH_CLI=true \
    GIT_CONFIG_COUNT=1 \
    GIT_CONFIG_KEY_0="url.file://${core}.insteadOf" \
    GIT_CONFIG_VALUE_0="https://github.com/kalapowered/kalareach.git" \
    "$@"
