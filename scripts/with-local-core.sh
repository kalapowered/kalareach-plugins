#!/usr/bin/env bash
#
# Runs a command with the kalareach core repository fetched from a local checkout.
#
#   scripts/with-local-core.sh cargo test
#   KALAREACH_CORE=/path/to/kalareach scripts/with-local-core.sh cargo run -p kalareach-catalogue -- validate
#
# The pipeline depends on kr-plugin-sdk by Git URL and exact revision, which is what pins the
# package contract this catalogue is built against. This rewrites the fetch URL for the duration of
# one command so Cargo reads a local checkout instead of the network, which is what you want when
# the SDK and the catalogue are being changed together, and what you want offline.
#
# The revision is unchanged. Cargo resolves the same commit object either way, and Cargo.toml and
# Cargo.lock keep the canonical URL, so what this builds is what a network fetch builds.
#
# The rewrite lives in this command's environment and nowhere else. It replaces any inherited
# GIT_CONFIG_* entries for the duration, and it applies to every Git subprocess the command starts.

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
