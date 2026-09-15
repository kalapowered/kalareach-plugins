#!/usr/bin/env bash
#
# Generates a set of development signing keys for the catalogue pipeline.
#
#   scripts/generate-development-keys.sh
#   scripts/generate-development-keys.sh /path/outside/the/repository
#
# Four keys, one per TUF role. Separate keys per role are the point of the design: a compromised
# timestamp key lets an attacker hold a client on an old generation, and nothing more.
#
# The keys are written outside this repository and the script refuses to write inside it. A key
# that reaches a commit has to be rotated, so the safest place for one is somewhere a commit cannot
# reach. The pipeline runs the same check before it signs anything.
#
# These are development keys. Production keys are generated in the signing environment, on the
# machine that will hold them, and never leave it.

set -euo pipefail

repository_root="$(cd "$(dirname "$0")/.." && pwd -P)"
default_dir="${XDG_DATA_HOME:-$HOME/.local/share}/kalareach-plugins/signing"
target="${1:-$default_dir}"

if ! command -v openssl >/dev/null 2>&1; then
    echo "openssl is required to generate keys" >&2
    exit 1
fi

mkdir -p "$target"
# -P resolves every symlink, so a directory that points inside the repository is caught by the
# containment check below rather than by the logical path it was typed as.
target="$(cd "$target" && pwd -P)"

case "$target/" in
    "$repository_root"/*)
        echo "$target is inside $repository_root; signing keys live outside the repository" >&2
        exit 1
        ;;
esac

chmod 700 "$target"

# Each role gets its own key. Creation is exclusive, so a dangling symlink left in place of a key
# file cannot make OpenSSL write through it to somewhere else.
set -C
for role in root targets snapshot timestamp; do
    key="$target/$role.pem"
    if [ -e "$key" ] || [ -L "$key" ]; then
        echo "$key exists; remove it first to replace the $role key" >&2
        exit 1
    fi
    if ! : > "$key"; then
        echo "$key could not be created" >&2
        exit 1
    fi
    chmod 600 "$key"
    openssl genpkey -algorithm RSA -pkeyopt rsa_keygen_bits:3072 -out "$key" 2>/dev/null
    chmod 600 "$key"
    echo "wrote $key"
done
set +C

cat <<EOF

Four development keys are in $target.

Write the trust root over them:

    cargo run -p kalareach-catalogue -- root --signing-dir "$target"

Then build a generation:

    cargo run -p kalareach-catalogue -- build --signing-dir "$target" --out /tmp/kalareach-catalogue-build
EOF
